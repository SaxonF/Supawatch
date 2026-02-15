//! Function introspection.

use crate::schema::{FunctionGrant, FunctionInfo};
use crate::supabase_api::SupabaseApi;
use serde::Deserialize;
use std::collections::HashMap;

use super::helpers::parse_function_args;

/// Parse config params from PostgreSQL proconfig array format
/// e.g., ["search_path=''", "statement_timeout=5000"]
fn parse_config_params(config: Option<Vec<String>>) -> Vec<(String, String)> {
    config.unwrap_or_default()
        .into_iter()
        .filter_map(|s| {
            let parts: Vec<&str> = s.splitn(2, '=').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

/// Parse grants from PostgreSQL proacl format using aclexplode results
/// Each grant has grantee (role name) and privilege_type
fn parse_grants(grants_json: Option<serde_json::Value>) -> Vec<FunctionGrant> {
    let Some(val) = grants_json else { return vec![] };
    
    // grants_json is an array of {grantee, privilege}
    if let serde_json::Value::Array(arr) = val {
        arr.into_iter()
            .filter_map(|item| {
                let grantee = item.get("grantee")?.as_str()?.to_string();
                let privilege = item.get("privilege")?.as_str()?.to_string();
                Some(FunctionGrant { grantee, privilege })
            })
            .collect()
    } else {
        vec![]
    }
}

/// Fetch all functions from the database.
pub async fn get_functions(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<HashMap<String, FunctionInfo>, String> {
    // Main query for function metadata including grants via aclexplode
    let query = r#"
        SELECT
          n.nspname as schema,
          p.proname as name,
          pg_get_function_result(p.oid) as return_type,
          pg_get_function_arguments(p.oid) as args,
          l.lanname as language,
          p.prosrc as definition,
          CASE p.provolatile
            WHEN 'i' THEN 'IMMUTABLE'
            WHEN 's' THEN 'STABLE'
            WHEN 'v' THEN 'VOLATILE'
          END as volatility,
          p.proisstrict as is_strict,
          p.prosecdef as security_definer,
          p.proconfig as config_params,
          ext.extname as extension,
          (
            SELECT jsonb_agg(jsonb_build_object(
              'grantee', COALESCE(r.rolname, 'public'),
              'privilege', acl.privilege_type
            ))
            FROM aclexplode(p.proacl) acl
            LEFT JOIN pg_roles r ON r.oid = acl.grantee
            WHERE acl.privilege_type = 'EXECUTE'
          ) as grants
        FROM pg_proc p
        JOIN pg_language l ON p.prolang = l.oid
        JOIN pg_namespace n ON p.pronamespace = n.oid
        LEFT JOIN pg_depend dep ON dep.objid = p.oid AND dep.classid = 'pg_proc'::regclass AND dep.deptype = 'e'
        LEFT JOIN pg_extension ext ON dep.refobjid = ext.oid AND dep.refclassid = 'pg_extension'::regclass
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
    "#;

    #[derive(Deserialize)]
    struct Row {
        schema: String,
        name: String,
        return_type: String,
        args: String,
        language: String,
        definition: String,
        volatility: Option<String>,
        is_strict: bool,
        security_definer: bool,
        config_params: Option<Vec<String>>,
        extension: Option<String>,
        grants: Option<serde_json::Value>,
    }

    let result = api
        .run_query(project_ref, query, true)
        .await
        .map_err(|e| e.to_string())?;

    let rows: Vec<Row> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| e.to_string())?;

    let mut functions = HashMap::new();
    for row in rows {
        let args = parse_function_args(&row.args);
        let arg_types: Vec<String> = args.iter().map(|a| a.type_.clone()).collect();
        let signature = format!("\"{}\".\"{}\"{}", row.schema, row.name, format!("({})", arg_types.join(", ")));

        functions.insert(
            signature,
            FunctionInfo {
                schema: row.schema,
                name: row.name,
                args,
                return_type: row.return_type,
                language: row.language,
                definition: row.definition,
                volatility: row.volatility,
                is_strict: row.is_strict,
                security_definer: row.security_definer,
                config_params: parse_config_params(row.config_params),
                grants: parse_grants(row.grants),
                extension: row.extension,
            },
        );
    }

    Ok(functions)
}
