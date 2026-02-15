//! View introspection (including materialized views).

use crate::schema::{IndexInfo, ViewColumnInfo, ViewInfo};
use crate::supabase_api::SupabaseApi;
use serde::Deserialize;
use std::collections::HashMap;

use super::helpers::parse_pg_array;

/// Fetch all views (regular and materialized) from the database.
pub async fn get_views(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<HashMap<String, ViewInfo>, String> {
    let query = r#"
        WITH view_data AS (
            -- Regular views
            SELECT
                n.nspname as schema,
                c.relname as name,
                pg_get_viewdef(c.oid, false) as definition,
                false as is_materialized,
                obj_description(c.oid, 'pg_class') as comment,
                c.reloptions as options,
                c.oid,
                ext.extname as extension
            FROM pg_class c
            JOIN pg_namespace n ON n.oid = c.relnamespace
            LEFT JOIN pg_depend dep ON dep.objid = c.oid AND dep.classid = 'pg_class'::regclass AND dep.deptype = 'e'
            LEFT JOIN pg_extension ext ON dep.refobjid = ext.oid AND dep.refclassid = 'pg_extension'::regclass
            WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
              AND n.nspname NOT LIKE 'pg_toast%'
              AND n.nspname NOT LIKE 'pg_temp%'
              AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
            AND c.relkind = 'v'

            UNION ALL

            -- Materialized views
            SELECT
                n.nspname as schema,
                c.relname as name,
                pg_get_viewdef(c.oid, false) as definition,
                true as is_materialized,
                obj_description(c.oid, 'pg_class') as comment,
                c.reloptions as options,
                c.oid,
                ext.extname as extension
            FROM pg_class c
            JOIN pg_namespace n ON n.oid = c.relnamespace
            LEFT JOIN pg_depend dep ON dep.objid = c.oid AND dep.classid = 'pg_class'::regclass AND dep.deptype = 'e'
            LEFT JOIN pg_extension ext ON dep.refobjid = ext.oid AND dep.refclassid = 'pg_extension'::regclass
            WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
              AND n.nspname NOT LIKE 'pg_toast%'
              AND n.nspname NOT LIKE 'pg_temp%'
              AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
            AND c.relkind = 'm'
        ),
        view_columns AS (
            SELECT
                n.nspname as schema,
                c.relname as view_name,
                a.attname as column_name,
                format_type(a.atttypid, a.atttypmod) as data_type,
                col_description(c.oid, a.attnum) as comment
            FROM pg_class c
            JOIN pg_namespace n ON n.oid = c.relnamespace
            JOIN pg_attribute a ON a.attrelid = c.oid
            WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
              AND n.nspname NOT LIKE 'pg_toast%'
              AND n.nspname NOT LIKE 'pg_temp%'
              AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
            AND c.relkind IN ('v', 'm')
            AND a.attnum > 0
            AND NOT a.attisdropped
        ),
        mat_view_indexes AS (
            SELECT
                n.nspname as schema,
                t.relname as view_name,
                i.relname as index_name,
                array_agg(a.attname ORDER BY array_position(ix.indkey, a.attnum)) as columns,
                ix.indisunique as is_unique,
                am.amname as index_method,
                pg_get_expr(ix.indpred, ix.indrelid) as where_clause
            FROM pg_class t
            JOIN pg_index ix ON t.oid = ix.indrelid
            JOIN pg_class i ON i.oid = ix.indexrelid
            JOIN pg_am am ON i.relam = am.oid
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
            JOIN pg_namespace n ON t.relnamespace = n.oid
            WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
              AND n.nspname NOT LIKE 'pg_toast%'
              AND n.nspname NOT LIKE 'pg_temp%'
              AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
            AND t.relkind = 'm'
            GROUP BY n.nspname, t.relname, i.relname, ix.indisunique, am.amname, ix.indpred, ix.indrelid
        )
        SELECT json_build_object(
            'views', (SELECT json_agg(row_to_json(view_data)) FROM view_data),
            'columns', (SELECT json_agg(row_to_json(view_columns)) FROM view_columns),
            'indexes', (SELECT json_agg(row_to_json(mat_view_indexes)) FROM mat_view_indexes)
        ) as data
    "#;

    let result = api
        .run_query(project_ref, query, true)
        .await
        .map_err(|e| format!("Views query failed: {}", e))?;

    let rows: Vec<serde_json::Value> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| e.to_string())?;

    let data = rows
        .first()
        .and_then(|r| r.get("data"))
        .cloned()
        .unwrap_or(serde_json::json!({}));

    parse_views_response(&data)
}

/// Parse the views response JSON into ViewInfo structs.
fn parse_views_response(data: &serde_json::Value) -> Result<HashMap<String, ViewInfo>, String> {
    #[derive(Deserialize)]
    struct ViewRow {
        schema: String,
        name: String,
        definition: Option<String>,
        is_materialized: bool,
        comment: Option<String>,
        options: Option<serde_json::Value>,
        extension: Option<String>,
    }

    #[derive(Deserialize)]
    struct ColumnRow {
        schema: String,
        view_name: String,
        column_name: String,
        data_type: String,
        comment: Option<String>,
    }

    #[derive(Deserialize)]
    struct IndexRow {
        schema: String,
        view_name: String,
        index_name: String,
        columns: serde_json::Value,
        is_unique: bool,
        index_method: String,
        where_clause: Option<String>,
    }

    let view_rows: Vec<ViewRow> = data
        .get("views")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let column_rows: Vec<ColumnRow> = data
        .get("columns")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let index_rows: Vec<IndexRow> = data
        .get("indexes")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    let mut views: HashMap<String, ViewInfo> = HashMap::new();

    for row in view_rows {
        let options = row.options.map(|v| parse_pg_array(&v)).unwrap_or_default();
        let key = format!("\"{}\".\"{}\"", row.schema, row.name);

        views.insert(
            key,
            ViewInfo {
                schema: row.schema,
                name: row.name,
                definition: row.definition.unwrap_or_default(),
                is_materialized: row.is_materialized,
                columns: vec![],
                indexes: vec![],
                comment: row.comment,
                with_options: options,
                check_option: None,
                extension: row.extension,
            },
        );
    }

    // Add columns to views
    for col in column_rows {
        let key = format!("\"{}\".\"{}\"", col.schema, col.view_name);
        if let Some(view) = views.get_mut(&key) {
            view.columns.push(ViewColumnInfo {
                name: col.column_name,
                data_type: col.data_type,
                comment: col.comment,
            });
        }
    }

    // Add indexes to materialized views
    for idx in index_rows {
        let key = format!("\"{}\".\"{}\"", idx.schema, idx.view_name);
        if let Some(view) = views.get_mut(&key) {
            view.indexes.push(IndexInfo {
                index_name: idx.index_name,
                columns: parse_pg_array(&idx.columns),
                is_unique: idx.is_unique,
                is_primary: false,
                owning_constraint: None,
                index_method: idx.index_method,
                where_clause: idx.where_clause,
                expressions: vec![],
            });
        }
    }

    Ok(views)
}
