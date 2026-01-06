//! Database type introspection: enums, composite types, and domains.

use crate::schema::{
    CompositeTypeAttribute, CompositeTypeInfo, DomainCheckConstraint, DomainInfo, EnumInfo,
};
use crate::supabase_api::SupabaseApi;
use serde::Deserialize;
use std::collections::HashMap;

use super::helpers::parse_pg_array;

/// Fetch enum types from the database.
pub async fn get_enums(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<HashMap<String, EnumInfo>, String> {
    let query = r#"
        SELECT n.nspname as schema, t.typname as name, array_agg(e.enumlabel ORDER BY e.enumsortorder) as values
        FROM pg_type t
        JOIN pg_enum e ON t.oid = e.enumtypid
        JOIN pg_namespace n ON t.typnamespace = n.oid
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
        GROUP BY n.nspname, t.typname
    "#;

    #[derive(Deserialize)]
    struct Row {
        schema: String,
        name: String,
        values: serde_json::Value,
    }

    let result = api
        .run_query(project_ref, query, true)
        .await
        .map_err(|e| e.to_string())?;

    let rows: Vec<Row> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| e.to_string())?;

    let mut enums = HashMap::new();
    for row in rows {
        let values = parse_pg_array(&row.values);
        let key = format!("\"{}\".\"{}\"", row.schema, row.name);
        enums.insert(
            key,
            EnumInfo {
                schema: row.schema,
                name: row.name,
                values,
            },
        );
    }

    Ok(enums)
}

/// Fetch composite types from the database.
pub async fn get_composite_types(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<HashMap<String, CompositeTypeInfo>, String> {
    let query = r#"
        SELECT
            n.nspname as schema,
            t.typname as name,
            array_agg(
                json_build_object(
                    'name', a.attname,
                    'data_type', format_type(a.atttypid, a.atttypmod),
                    'collation', c.collname
                ) ORDER BY a.attnum
            ) as attributes,
            obj_description(t.oid, 'pg_type') as comment
        FROM pg_type t
        JOIN pg_namespace n ON t.typnamespace = n.oid
        JOIN pg_class cls ON cls.oid = t.typrelid
        LEFT JOIN pg_attribute a ON a.attrelid = cls.oid AND a.attnum > 0 AND NOT a.attisdropped
        LEFT JOIN pg_collation c ON c.oid = a.attcollation AND c.collname != 'default'
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
        AND t.typtype = 'c'
        AND cls.relkind = 'c'
        GROUP BY n.nspname, t.typname, t.oid
    "#;

    #[derive(Deserialize)]
    struct Row {
        schema: String,
        name: String,
        attributes: serde_json::Value,
        comment: Option<String>,
    }

    let result = api
        .run_query(project_ref, query, true)
        .await
        .map_err(|e| e.to_string())?;

    let rows: Vec<Row> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| e.to_string())?;

    let mut types = HashMap::new();
    for row in rows {
        let attrs: Vec<CompositeTypeAttribute> = if let Some(arr) = row.attributes.as_array() {
            arr.iter()
                .filter_map(|v| {
                    Some(CompositeTypeAttribute {
                        name: v.get("name")?.as_str()?.to_string(),
                        data_type: v.get("data_type")?.as_str()?.to_string(),
                        collation: v
                            .get("collation")
                            .and_then(|c| c.as_str())
                            .map(String::from),
                    })
                })
                .collect()
        } else {
            vec![]
        };

        let key = format!("\"{}\".\"{}\"", row.schema, row.name);
        types.insert(
            key,
            CompositeTypeInfo {
                schema: row.schema,
                name: row.name,
                attributes: attrs,
                comment: row.comment,
            },
        );
    }

    Ok(types)
}

/// Fetch domain types from the database.
pub async fn get_domains(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<HashMap<String, DomainInfo>, String> {
    let query = r#"
        SELECT
            n.nspname as schema,
            t.typname as name,
            format_type(t.typbasetype, t.typtypmod) as base_type,
            t.typdefault as default_value,
            t.typnotnull as is_not_null,
            c.collname as collation,
            obj_description(t.oid, 'pg_type') as comment,
            (
                SELECT json_agg(json_build_object(
                    'name', con.conname,
                    'expression', pg_get_constraintdef(con.oid)
                ))
                FROM pg_constraint con
                WHERE con.contypid = t.oid
            ) as check_constraints
        FROM pg_type t
        JOIN pg_namespace n ON t.typnamespace = n.oid
        LEFT JOIN pg_collation c ON c.oid = t.typcollation AND c.collname != 'default'
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
        AND t.typtype = 'd'
    "#;

    #[derive(Deserialize)]
    struct Row {
        schema: String,
        name: String,
        base_type: String,
        default_value: Option<String>,
        is_not_null: bool,
        collation: Option<String>,
        comment: Option<String>,
        check_constraints: Option<serde_json::Value>,
    }

    let result = api
        .run_query(project_ref, query, true)
        .await
        .map_err(|e| e.to_string())?;

    let rows: Vec<Row> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| e.to_string())?;

    let mut domains = HashMap::new();
    for row in rows {
        let checks: Vec<DomainCheckConstraint> = row
            .check_constraints
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|c| {
                Some(DomainCheckConstraint {
                    name: c.get("name").and_then(|n| n.as_str()).map(String::from),
                    expression: c.get("expression")?.as_str()?.to_string(),
                })
            })
            .collect();

        let key = format!("\"{}\".\"{}\"", row.schema, row.name);
        domains.insert(
            key,
            DomainInfo {
                schema: row.schema,
                name: row.name,
                base_type: row.base_type,
                default_value: row.default_value,
                is_not_null: row.is_not_null,
                check_constraints: checks,
                collation: row.collation,
                comment: row.comment,
            },
        );
    }

    Ok(domains)
}
