use crate::schema::{DefaultPrivilege, ObjectGrant, SchemaGrant};
use crate::supabase_api::SupabaseApi;
use serde::Deserialize;

use super::queries::{DEFAULT_PRIVILEGES_QUERY, OBJECT_GRANTS_QUERY, SCHEMA_GRANTS_QUERY};

#[derive(Deserialize)]
struct SchemaGrantRow {
    schema: String,
    grantee: String,
    privilege: String,
}

#[derive(Deserialize)]
struct DefaultPrivilegeRow {
    schema: String,
    object_type: String,
    grantee: String,
    privilege: String,
}

pub async fn get_schema_grants(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<Vec<SchemaGrant>, String> {
    let result = api
        .run_query(project_ref, SCHEMA_GRANTS_QUERY, true)
        .await
        .map_err(|e| format!("Failed to fetch schema grants: {}", e))?;

    let rows: Vec<SchemaGrantRow> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| format!("Failed to parse schema grants: {}", e))?;

    let mut grants = Vec::new();
    for row in rows {
        grants.push(SchemaGrant {
            schema: row.schema,
            grantee: row.grantee,
            privilege: row.privilege,
        });
    }

    Ok(grants)
}

pub async fn get_default_privileges(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<Vec<DefaultPrivilege>, String> {
    let result = api
        .run_query(project_ref, DEFAULT_PRIVILEGES_QUERY, true)
        .await
        .map_err(|e| format!("Failed to fetch default privileges: {}", e))?;

    let rows: Vec<DefaultPrivilegeRow> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| format!("Failed to parse default privileges: {}", e))?;

    let mut privs = Vec::new();
    for row in rows {
        privs.push(DefaultPrivilege {
            schema: row.schema,
            object_type: row.object_type,
            grantee: row.grantee,
            privilege: row.privilege,
        });
    }

    Ok(privs)
}

#[derive(Deserialize)]
struct ObjectGrantRow {
    schema: String,
    object_name: String,
    object_type: String,
    grantee: String,
    privilege: String,
}

/// Fetch grants on tables, views, and sequences.
/// Returns a vec of (object_type, qualified_key, ObjectGrant).
pub async fn get_object_grants(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<Vec<(String, String, ObjectGrant)>, String> {
    let result = api
        .run_query(project_ref, OBJECT_GRANTS_QUERY, true)
        .await
        .map_err(|e| format!("Failed to fetch object grants: {}", e))?;

    let rows: Vec<ObjectGrantRow> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| format!("Failed to parse object grants: {}", e))?;

    let mut grants = Vec::new();
    for row in rows {
        // Filter out system grantees
        if row.grantee == "postgres" || row.grantee == "supabase_admin" {
            continue;
        }

        let key = format!("\"{}\".\"{}\"", row.schema, row.object_name);
        grants.push((
            row.object_type,
            key,
            ObjectGrant {
                grantee: row.grantee,
                privilege: row.privilege,
            },
        ));
    }

    Ok(grants)
}
