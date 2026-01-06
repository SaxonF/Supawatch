//! Sequence introspection.

use crate::schema::SequenceInfo;
use crate::supabase_api::SupabaseApi;
use serde::Deserialize;
use std::collections::HashMap;

use super::helpers::deserialize_i64_or_string;

/// Fetch all sequences from the database.
pub async fn get_sequences(
    api: &SupabaseApi,
    project_ref: &str,
) -> Result<HashMap<String, SequenceInfo>, String> {
    let query = r#"
        SELECT
            n.nspname as schema,
            s.relname as name,
            format_type(seq.seqtypid, NULL) as data_type,
            seq.seqstart as start_value,
            seq.seqmin as min_value,
            seq.seqmax as max_value,
            seq.seqincrement as increment,
            seq.seqcycle as cycle,
            seq.seqcache as cache_size,
            CASE WHEN d.refobjid IS NOT NULL
                THEN c.relname || '.' || a.attname
                ELSE NULL
            END as owned_by,
            obj_description(s.oid, 'pg_class') as comment
        FROM pg_class s
        JOIN pg_sequence seq ON seq.seqrelid = s.oid
        JOIN pg_namespace n ON n.oid = s.relnamespace
        LEFT JOIN pg_depend d ON d.objid = s.oid AND d.deptype = 'a'
        LEFT JOIN pg_class c ON c.oid = d.refobjid
        LEFT JOIN pg_attribute a ON a.attrelid = d.refobjid AND a.attnum = d.refobjsubid
        WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
          AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron')
        AND s.relkind = 'S'
    "#;

    #[derive(Deserialize)]
    struct Row {
        schema: String,
        name: String,
        data_type: String,
        #[serde(deserialize_with = "deserialize_i64_or_string")]
        start_value: i64,
        #[serde(deserialize_with = "deserialize_i64_or_string")]
        min_value: i64,
        #[serde(deserialize_with = "deserialize_i64_or_string")]
        max_value: i64,
        #[serde(deserialize_with = "deserialize_i64_or_string")]
        increment: i64,
        cycle: bool,
        #[serde(deserialize_with = "deserialize_i64_or_string")]
        cache_size: i64,
        owned_by: Option<String>,
        comment: Option<String>,
    }

    let result = api
        .run_query(project_ref, query, true)
        .await
        .map_err(|e| e.to_string())?;

    let rows: Vec<Row> =
        serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
            .map_err(|e| e.to_string())?;

    let mut sequences = HashMap::new();
    for row in rows {
        let key = format!("\"{}\".\"{}\"", row.schema, row.name);
        sequences.insert(
            key,
            SequenceInfo {
                schema: row.schema,
                name: row.name,
                data_type: row.data_type,
                start_value: row.start_value,
                min_value: row.min_value,
                max_value: row.max_value,
                increment: row.increment,
                cycle: row.cycle,
                cache_size: row.cache_size,
                owned_by: row.owned_by,
                comment: row.comment,
            },
        );
    }

    Ok(sequences)
}
