//! Function introspection.

use crate::schema::{FunctionArg, FunctionInfo};
use crate::supabase_api::SupabaseApi;
use serde::Deserialize;
use std::collections::HashMap;

use super::helpers::parse_function_args;

/// Fetch all functions from the database.
pub async fn get_functions(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<HashMap<String, FunctionInfo>, String> {
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
          p.prosecdef as security_definer
        FROM pg_proc p
        JOIN pg_language l ON p.prolang = l.oid
        JOIN pg_namespace n ON p.pronamespace = n.oid
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron')
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
        let signature = format!("\"{}\".\"{}\"({})", row.schema, row.name, arg_types.join(", "));

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
            },
        );
    }

    Ok(functions)
}
