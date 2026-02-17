//! Table introspection: get_all_tables_bulk and parse_bulk_response.

use crate::schema::{
    CheckConstraintInfo, ColumnInfo, ForeignKeyInfo, IndexInfo, PolicyInfo, TableInfo, TriggerInfo,
};
use crate::supabase_api::SupabaseApi;
use serde::Deserialize;
use std::collections::HashMap;

use super::helpers::{extract_index_expressions, extract_trigger_when_clause, extract_update_of_columns, parse_pg_array, parse_policy_cmd};

/// The bulk SQL query to fetch all table information in a single call.
pub const TABLES_BULK_QUERY: &str = r#"
    WITH table_list AS (
        SELECT
            n.nspname as schema,
            c.relname as name,
            ext.extname as extension
        FROM pg_class c
        JOIN pg_namespace n ON c.relnamespace = n.oid
        LEFT JOIN pg_depend dep ON dep.objid = c.oid AND dep.classid = 'pg_class'::regclass AND dep.deptype = 'e'
        LEFT JOIN pg_extension ext ON dep.refobjid = ext.oid AND dep.refclassid = 'pg_extension'::regclass
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
        AND c.relkind = 'r'
    ),
    columns_data AS (
        SELECT
            n.nspname as schema,
            t.relname as table_name,
            a.attname as column_name,
            format_type(a.atttypid, a.atttypmod) as data_type,
            CASE WHEN a.attnotnull THEN 'NO' ELSE 'YES' END as is_nullable,
            pg_get_expr(d.adbin, d.adrelid) as column_default,
            t_type.typname as udt_name,
            CASE a.attidentity
                WHEN 'a' THEN 'ALWAYS'
                WHEN 'd' THEN 'BY DEFAULT'
                ELSE NULL
            END as identity_generation,
            CASE WHEN a.attidentity != '' THEN 'YES' ELSE 'NO' END as is_identity,
            coll.collname as collation,
            COALESCE(pk.is_primary, false) as is_primary_key,

            false as is_unique,
            a.attgenerated as generated_status,
            CASE WHEN a.attgenerated = 's' THEN pg_get_expr(d.adbin, d.adrelid) ELSE NULL END as generation_expression,
            col_description(t.oid, a.attnum) as comment
        FROM pg_attribute a
        JOIN pg_class t ON a.attrelid = t.oid
        JOIN pg_namespace n ON t.relnamespace = n.oid
        JOIN pg_type t_type ON a.atttypid = t_type.oid
        LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
        LEFT JOIN pg_collation coll ON coll.oid = a.attcollation AND coll.collname != 'default'
        LEFT JOIN (
            SELECT c.conrelid, unnest(c.conkey) as attnum, true as is_primary
            FROM pg_constraint c
            WHERE c.contype = 'p'
        ) pk ON pk.conrelid = a.attrelid AND pk.attnum = a.attnum
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
        AND t.relkind = 'r'
        AND a.attnum > 0
        AND NOT a.attisdropped
    ),
    fk_data AS (
        SELECT
            n.nspname as schema,
            c.relname as table_name,
            con.conname as constraint_name,
            a.attname as column_name,
            nf.nspname as foreign_schema,
            cf.relname as foreign_table,
            af.attname as foreign_column,
            CASE con.confdeltype
                WHEN 'a' THEN 'NO ACTION'
                WHEN 'r' THEN 'RESTRICT'
                WHEN 'c' THEN 'CASCADE'
                WHEN 'n' THEN 'SET NULL'
                WHEN 'd' THEN 'SET DEFAULT'
                ELSE 'NO ACTION'
            END as on_delete,
            CASE con.confupdtype
                WHEN 'a' THEN 'NO ACTION'
                WHEN 'r' THEN 'RESTRICT'
                WHEN 'c' THEN 'CASCADE'
                WHEN 'n' THEN 'SET NULL'
                WHEN 'd' THEN 'SET DEFAULT'
                ELSE 'NO ACTION'
            END as on_update
        FROM pg_constraint con
        JOIN pg_class c ON con.conrelid = c.oid
        JOIN pg_namespace n ON c.relnamespace = n.oid
        JOIN pg_class cf ON con.confrelid = cf.oid
        JOIN pg_namespace nf ON cf.relnamespace = nf.oid
        CROSS JOIN LATERAL unnest(con.conkey, con.confkey) AS k(con_attnum, conf_attnum)
        JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = k.con_attnum
        JOIN pg_attribute af ON af.attrelid = cf.oid AND af.attnum = k.conf_attnum
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
        AND con.contype = 'f'
    ),
    index_data AS (
        SELECT
            n.nspname as schema,
            t.relname as table_name,
            i.relname as index_name,
            array_agg(a.attname ORDER BY array_position(ix.indkey::int[], a.attnum)) FILTER (WHERE a.attname IS NOT NULL) as columns,
            ix.indisunique as is_unique,
            ix.indisprimary as is_primary,
            MAX(con.conname) as owning_constraint,
            am.amname as index_method,
            pg_get_expr(ix.indpred, ix.indrelid) as where_clause,
            pg_get_indexdef(i.oid) as index_def
        FROM pg_class t
        JOIN pg_index ix ON t.oid = ix.indrelid
        JOIN pg_class i ON i.oid = ix.indexrelid
        JOIN pg_am am ON i.relam = am.oid
        LEFT JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey::int[]) AND a.attnum > 0 AND NOT a.attisdropped
        JOIN pg_namespace n ON t.relnamespace = n.oid
        LEFT JOIN pg_constraint con ON con.conindid = i.oid
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
        AND NOT ix.indisprimary
        GROUP BY n.nspname, t.relname, i.relname, ix.indisunique, ix.indisprimary, am.amname, ix.indpred, ix.indrelid, i.oid
    ),
    trigger_data AS (
        SELECT
            n.nspname as schema,
            c.relname as table_name,
            t.tgname as trigger_name,
            t.tgtype::integer as tgtype,
            p.proname as function_name,
            pg_get_triggerdef(t.oid) as trigger_def
        FROM pg_trigger t
        JOIN pg_class c ON c.oid = t.tgrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        JOIN pg_proc p ON p.oid = t.tgfoid
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
        AND NOT t.tgisinternal
    ),
    policy_data AS (
        SELECT
            n.nspname as schema,
            c.relname as table_name,
            p.polname as name,
            p.polcmd as cmd,
            CASE
                WHEN p.polroles = '{0}' THEN ARRAY['public']
                ELSE ARRAY(SELECT rolname::text FROM pg_authid WHERE oid = ANY(p.polroles))
            END as roles,
            pg_get_expr(p.polqual, p.polrelid) as qual,
            pg_get_expr(p.polwithcheck, p.polrelid) as with_check
        FROM pg_policy p
        JOIN pg_class c ON c.oid = p.polrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
    ),
    rls_data AS (
        SELECT n.nspname as schema, c.relname as table_name, c.relrowsecurity as rls_enabled
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
        AND c.relkind = 'r'
    ),
    check_data AS (
        SELECT
            n.nspname as schema,
            c.relname as table_name,
            con.conname as name,
            pg_get_constraintdef(con.oid) as expression,
            array_agg(a.attname ORDER BY a.attnum) as columns
        FROM pg_constraint con
        JOIN pg_class c ON con.conrelid = c.oid
        JOIN pg_namespace n ON c.relnamespace = n.oid
        LEFT JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = ANY(con.conkey)
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
        AND con.contype = 'c'
        GROUP BY n.nspname, c.relname, con.conname, con.oid
    ),
    table_comments AS (
        SELECT
            n.nspname as schema,
            c.relname as table_name,
            obj_description(c.oid, 'pg_class') as comment
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
        AND c.relkind = 'r'
    )
    SELECT json_build_object(
        'tables', (SELECT json_agg(row_to_json(table_list)) FROM table_list),
        'columns', (SELECT json_agg(row_to_json(columns_data)) FROM columns_data),
        'foreign_keys', (SELECT json_agg(row_to_json(fk_data)) FROM fk_data),
        'indexes', (SELECT json_agg(row_to_json(index_data)) FROM index_data),
        'triggers', (SELECT json_agg(row_to_json(trigger_data)) FROM trigger_data),
        'policies', (SELECT json_agg(row_to_json(policy_data)) FROM policy_data),
        'rls', (SELECT json_agg(row_to_json(rls_data)) FROM rls_data),
        'check_constraints', (SELECT json_agg(row_to_json(check_data)) FROM check_data),
        'table_comments', (SELECT json_agg(row_to_json(table_comments)) FROM table_comments)
    ) as data
"#;

/// Fetch all table information using a bulk query (minimal API calls).
pub async fn get_all_tables_bulk(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<HashMap<String, TableInfo>, String> {
    let result = api
        .run_query(project_ref, TABLES_BULK_QUERY, true)
        .await
        .map_err(|e| format!("Bulk query failed: {}", e))?;

    let rows: Vec<serde_json::Value> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| e.to_string())?;

    let data = rows
        .first()
        .and_then(|r| r.get("data"))
        .cloned()
        .unwrap_or(serde_json::json!({}));

    parse_bulk_response(&data)
}

/// Parse the bulk response JSON into TableInfo structs.
pub fn parse_bulk_response(data: &serde_json::Value) -> Result<HashMap<String, TableInfo>, String> {
    #[derive(Deserialize)]
    struct TableRow {
        schema: String,
        name: String,
        extension: Option<String>,
    }
    let table_rows: Vec<TableRow> = data
        .get("tables")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    #[derive(Deserialize)]
    struct ColumnRow {
        schema: String,
        table_name: String,
        column_name: String,
        data_type: String,
        is_nullable: String,
        column_default: Option<String>,
        udt_name: String,
        is_identity: String,
        identity_generation: Option<String>,
        collation: Option<String>,
        is_primary_key: bool,
        is_unique: bool,
        generated_status: Option<String>,
        generation_expression: Option<String>,
        comment: Option<String>,
    }
    let columns: Vec<ColumnRow> = data
        .get("columns")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    #[derive(Deserialize)]
    struct FkRow {
        schema: String,
        table_name: String,
        constraint_name: String,
        column_name: String,
        foreign_schema: String,
        foreign_table: String,
        foreign_column: String,
        on_delete: String,
        on_update: String,
    }
    let fks: Vec<FkRow> = data
        .get("foreign_keys")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    #[derive(Deserialize)]
    struct IndexRow {
        schema: String,
        table_name: String,
        index_name: String,
        columns: serde_json::Value,
        is_unique: bool,
        is_primary: bool,
        owning_constraint: Option<String>,
        index_method: String,
        where_clause: Option<String>,
        index_def: Option<String>,
    }
    let indexes: Vec<IndexRow> = data
        .get("indexes")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    #[derive(Deserialize)]
    struct TriggerRow {
        schema: String,
        table_name: String,
        trigger_name: String,
        tgtype: i32,
        function_name: String,
        trigger_def: Option<String>,
    }
    let triggers: Vec<TriggerRow> = data
        .get("triggers")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    #[derive(Deserialize)]
    struct PolicyRow {
        schema: String,
        table_name: String,
        name: String,
        cmd: String,
        roles: Vec<String>,
        qual: Option<String>,
        with_check: Option<String>,
    }
    let policies: Vec<PolicyRow> = data
        .get("policies")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    #[derive(Deserialize)]
    struct RlsRow {
        schema: String,
        table_name: String,
        rls_enabled: bool,
    }
    let rls_data: Vec<RlsRow> = data
        .get("rls")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    #[derive(Deserialize)]
    struct CheckRow {
        schema: String,
        table_name: String,
        name: String,
        expression: String,
        columns: serde_json::Value,
    }
    let check_data: Vec<CheckRow> = data
        .get("check_constraints")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    #[derive(Deserialize)]
    struct CommentRow {
        schema: String,
        table_name: String,
        comment: Option<String>,
    }
    let comment_data: Vec<CommentRow> = data
        .get("table_comments")
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();

    // Build tables map
    let mut tables: HashMap<String, TableInfo> = HashMap::new();

    // Initialize all tables
    for row in table_rows {
        let key = format!("\"{}\".\"{}\"", row.schema, row.name);
        tables.insert(
            key,
            TableInfo {
                schema: row.schema,
                table_name: row.name,
                columns: HashMap::new(),
                foreign_keys: vec![],
                indexes: vec![],
                triggers: vec![],
                rls_enabled: false,
                policies: vec![],
                check_constraints: vec![],
                comment: None,
                extension: row.extension,
            },
        );
    }

    // Populate columns
    for col in columns {
        let key = format!("\"{}\".\"{}\"", col.schema, col.table_name);
        if let Some(table) = tables.get_mut(&key) {
            let mut final_data_type = col.data_type.clone();
            if final_data_type == "ARRAY" {
                if col.udt_name.starts_with('_') {
                    final_data_type = format!("{}[]", &col.udt_name[1..]);
                }
            }

            table.columns.insert(
                col.column_name.clone(),
                ColumnInfo {
                    column_name: col.column_name,
                    data_type: final_data_type.clone(),
                    is_nullable: col.is_nullable == "YES",
                    column_default: col.column_default,
                    udt_name: col.udt_name.clone(),
                    is_identity: col.is_identity == "YES",
                    identity_generation: col.identity_generation,
                    is_primary_key: col.is_primary_key,
                    is_unique: col.is_unique,
                    collation: col.collation,
                    enum_name: None,
                    is_array: final_data_type.ends_with("[]"),
                    is_generated: col.generated_status.as_deref() == Some("s"),
                    generation_expression: col.generation_expression,
                    comment: col.comment,
                },
            );
        }
    }

    // Populate foreign keys
    for fk in fks {
        let key = format!("\"{}\".\"{}\"", fk.schema, fk.table_name);
        if let Some(table) = tables.get_mut(&key) {
            table.foreign_keys.push(ForeignKeyInfo {
                constraint_name: fk.constraint_name,
                column_name: fk.column_name,
                foreign_schema: fk.foreign_schema,
                foreign_table: fk.foreign_table,
                foreign_column: fk.foreign_column,
                on_delete: fk.on_delete,
                on_update: fk.on_update,
            });
        }
    }

    // Populate indexes
    for idx in indexes {
        let key = format!("\"{}\".\"{}\"", idx.schema, idx.table_name);
        if let Some(table) = tables.get_mut(&key) {
            let expressions = idx
                .index_def
                .as_ref()
                .map(|d| extract_index_expressions(d))
                .unwrap_or_default();

            table.indexes.push(IndexInfo {
                index_name: idx.index_name,
                columns: parse_pg_array(&idx.columns),
                is_unique: idx.is_unique,
                is_primary: idx.is_primary,
                owning_constraint: idx.owning_constraint,
                index_method: idx.index_method,
                where_clause: idx.where_clause,
                expressions,
            });
        }
    }

    // Populate triggers - use tuple to store table_key with TriggerInfo
    let mut trigger_map: HashMap<String, (String, TriggerInfo)> = HashMap::new();
    for tr in triggers {
        let trig_key = format!(
            "\"{}\".\"{}\".{}",
            tr.schema, tr.table_name, tr.trigger_name
        );
        let table_key = format!("\"{}\".\"{}\"", tr.schema, tr.table_name);

        let tgtype = tr.tgtype;
        let is_row = (tgtype & 1) != 0;
        let is_before = (tgtype & 2) != 0;
        let is_insert = (tgtype & 4) != 0;
        let is_delete = (tgtype & 8) != 0;
        let is_update = (tgtype & 16) != 0;
        let is_truncate = (tgtype & 32) != 0;
        let is_instead = (tgtype & 64) != 0;

        let timing = if is_instead {
            "INSTEAD OF"
        } else if is_before {
            "BEFORE"
        } else {
            "AFTER"
        }
        .to_string();

        let orientation = if is_row { "ROW" } else { "STATEMENT" }.to_string();

        let mut events = vec![];
        if is_insert {
            events.push("INSERT".to_string());
        }
        if is_update {
            // Check for UPDATE OF columns in trigger_def
            if let Some(cols) = tr.trigger_def.as_ref().and_then(|d| extract_update_of_columns(d)) {
                // Format: "UPDATE OF \"col1\", \"col2\""
                let cols_formatted = cols.iter()
                    .map(|c| format!("\"{}\"" , c))
                    .collect::<Vec<_>>()
                    .join(", ");
                events.push(format!("UPDATE OF {}", cols_formatted));
            } else {
                events.push("UPDATE".to_string());
            }
        }
        if is_delete {
            events.push("DELETE".to_string());
        }
        if is_truncate {
            events.push("TRUNCATE".to_string());
        }

        let when_clause = tr
            .trigger_def
            .as_ref()
            .and_then(|d| extract_trigger_when_clause(d));

        trigger_map
            .entry(trig_key)
            .and_modify(|(_, existing)| {
                for e in &events {
                    if !existing.events.contains(e) {
                        existing.events.push(e.clone());
                    }
                }
            })
            .or_insert_with(|| {
                (table_key, TriggerInfo {
                    name: tr.trigger_name.clone(),
                    timing,
                    events,
                    orientation,
                    function_name: tr.function_name.clone(),
                    when_clause,
                })
            });
    }

    // Add triggers to tables
    for (_trig_key, (table_key, trig)) in trigger_map {
        if let Some(table) = tables.get_mut(&table_key) {
            table.triggers.push(trig);
        }
    }

    // Populate RLS
    for rls in rls_data {
        let key = format!("\"{}\".\"{}\"", rls.schema, rls.table_name);
        if let Some(table) = tables.get_mut(&key) {
            table.rls_enabled = rls.rls_enabled;
        }
    }

    // Populate policies
    for pol in policies {
        let key = format!("\"{}\".\"{}\"", pol.schema, pol.table_name);
        if let Some(table) = tables.get_mut(&key) {
            table.policies.push(PolicyInfo {
                name: pol.name,
                cmd: parse_policy_cmd(&pol.cmd),
                roles: pol.roles,
                qual: pol.qual,
                with_check: pol.with_check,
            });
        }
    }

    // Populate check constraints
    for check in check_data {
        let key = format!("\"{}\".\"{}\"", check.schema, check.table_name);
        if let Some(table) = tables.get_mut(&key) {
            table.check_constraints.push(CheckConstraintInfo {
                name: check.name,
                expression: check.expression,
                columns: parse_pg_array(&check.columns),
            });
        }
    }

    // Populate table comments
    for comment in comment_data {
        let key = format!("\"{}\".\"{}\"", comment.schema, comment.table_name);
        if let Some(table) = tables.get_mut(&key) {
            table.comment = comment.comment;
        }
    }

    Ok(tables)
}
