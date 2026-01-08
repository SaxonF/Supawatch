//! Roles and extensions introspection.

use crate::schema::{ExtensionInfo, RoleInfo};
use crate::supabase_api::SupabaseApi;
use serde::Deserialize;
use std::collections::HashMap;

/// Fetch database roles.
pub async fn get_roles(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<HashMap<String, RoleInfo>, String> {
    let query = r#"
        SELECT
            rolname as name,
            rolsuper as superuser,
            rolcreatedb as create_db,
            rolcreaterole as create_role,
            rolinherit as inherit,
            rolcanlogin as login,
            rolreplication as replication,
            rolbypassrls as bypass_rls,
            rolconnlimit as connection_limit,
            rolvaliduntil::text as valid_until
        FROM pg_roles
        WHERE rolname NOT LIKE 'pg_%'
          AND rolname NOT LIKE 'supabase%'
          AND rolname NOT IN ('postgres', 'authenticator', 'anon', 'service_role', 'dashboard_user', 'pgbouncer')
    "#;

    #[derive(Deserialize)]
    struct RoleRow {
        name: String,
        superuser: bool,
        create_db: bool,
        create_role: bool,
        inherit: bool,
        login: bool,
        replication: bool,
        bypass_rls: bool,
        connection_limit: i32,
        valid_until: Option<String>,
    }

    let result = api
        .run_query(project_ref, query, true)
        .await
        .map_err(|e| e.to_string())?;

    let rows: Vec<RoleRow> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| e.to_string())?;

    let mut roles = HashMap::new();
    for row in rows {
        roles.insert(
            row.name.clone(),
            RoleInfo {
                name: row.name,
                superuser: row.superuser,
                create_db: row.create_db,
                create_role: row.create_role,
                inherit: row.inherit,
                login: row.login,
                replication: row.replication,
                bypass_rls: row.bypass_rls,
                connection_limit: row.connection_limit,
                valid_until: row.valid_until,
                password: None,
            },
        );
    }

    Ok(roles)
}

/// Fetch database extensions.
pub async fn get_extensions(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<HashMap<String, ExtensionInfo>, String> {
    let query = r#"
        SELECT
            e.extname as name,
            e.extversion as version,
            n.nspname as schema
        FROM pg_extension e
        JOIN pg_namespace n ON n.oid = e.extnamespace
        WHERE e.extname != 'plpgsql'
    "#;

    #[derive(Deserialize)]
    struct Row {
        name: String,
        version: Option<String>,
        schema: Option<String>,
    }

    let result = api
        .run_query(project_ref, query, true)
        .await
        .map_err(|e| e.to_string())?;

    let rows: Vec<Row> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| e.to_string())?;

    let mut extensions = HashMap::new();
    for row in rows {
        extensions.insert(
            row.name.clone(),
            ExtensionInfo {
                name: row.name,
                version: row.version,
                schema: row.schema,
            },
        );
    }

    Ok(extensions)
}
