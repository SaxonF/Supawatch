use crate::schema::{
    CheckConstraintInfo, ColumnInfo, CompositeTypeAttribute, CompositeTypeInfo, DbSchema,
    DomainCheckConstraint, DomainInfo, EnumInfo, ExtensionInfo, ForeignKeyInfo, FunctionArg,
    FunctionInfo, IndexInfo, PolicyInfo, RoleInfo, SequenceInfo, TableInfo, TriggerInfo, ViewColumnInfo,
    ViewInfo,
};
use crate::supabase_api::SupabaseApi;
use serde::Deserialize;
use std::collections::HashMap;

pub struct Introspector<'a> {
    api: &'a SupabaseApi,
    project_ref: String,
}

impl<'a> Introspector<'a> {
    pub fn new(api: &'a SupabaseApi, project_ref: String) -> Self {
        Self { api, project_ref }
    }

    pub async fn introspect(&self) -> Result<DbSchema, String> {
        println!(
            "[DEBUG introspect] Starting introspection for project: {}",
            self.project_ref
        );

        // Run all bulk queries in parallel for maximum efficiency
        println!("[DEBUG introspect] Running bulk queries...");

        let (enums, functions, roles, tables_data, views, sequences, extensions, composite_types, domains) =
            tokio::try_join!(
                self.get_enums(),
                self.get_functions(),
                self.get_roles(),
                self.get_all_tables_bulk(),
                self.get_views(),
                self.get_sequences(),
                self.get_extensions(),
                self.get_composite_types(),
                self.get_domains()
            )?;

        let total_triggers: usize = tables_data.values().map(|t| t.triggers.len()).sum();
        let total_policies: usize = tables_data.values().map(|t| t.policies.len()).sum();

        println!(
            "[DEBUG introspect] Got {} enums, {} functions, {} tables",
            enums.len(),
            functions.len(),
            tables_data.len()
        );
        println!(
            "[DEBUG introspect] Got {} triggers, {} policies",
            total_triggers, total_policies
        );
        println!(
            "[DEBUG introspect] Got {} views, {} sequences, {} extensions",
            views.len(),
            sequences.len(),
            extensions.len()
        );
        println!(
            "[DEBUG introspect] Got {} composite types, {} domains",
            composite_types.len(),
            domains.len()
        );

        println!("[DEBUG introspect] Introspection complete!");
        Ok(DbSchema {
            tables: tables_data,
            enums,
            functions,
            roles,
            views,
            sequences,
            extensions,
            composite_types,
            domains,
        })
    }

    async fn get_enums(&self) -> Result<HashMap<String, EnumInfo>, String> {
        let query = r#"
            SELECT t.typname as name, array_agg(e.enumlabel ORDER BY e.enumsortorder) as values
            FROM pg_type t
            JOIN pg_enum e ON t.oid = e.enumtypid
            JOIN pg_namespace n ON t.typnamespace = n.oid
            WHERE n.nspname = 'public'
            GROUP BY t.typname
        "#;

        #[derive(Deserialize)]
        struct Row {
            name: String,
            values: serde_json::Value,
        }

        let result = self
            .api
            .run_query(&self.project_ref, query, true)
            .await
            .map_err(|e| e.to_string())?;

        let rows: Vec<Row> =
            serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
                .map_err(|e| e.to_string())?;

        let mut enums = HashMap::new();
        for row in rows {
            let values = parse_pg_array(&row.values);
            enums.insert(
                row.name.clone(),
                EnumInfo {
                    name: row.name,
                    values,
                },
            );
        }

        Ok(enums)
    }

    async fn get_functions(&self) -> Result<HashMap<String, FunctionInfo>, String> {
        let query = r#"
            SELECT
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
            WHERE n.nspname = 'public'
        "#;

        #[derive(Deserialize)]
        struct Row {
            name: String,
            return_type: String,
            args: String,
            language: String,
            definition: String,
            volatility: Option<String>,
            is_strict: bool,
            security_definer: bool,
        }

        let result = self
            .api
            .run_query(&self.project_ref, query, true)
            .await
            .map_err(|e| e.to_string())?;

        let rows: Vec<Row> =
            serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
                .map_err(|e| e.to_string())?;

        let mut functions = HashMap::new();
        for row in rows {
            let args = parse_function_args(&row.args);
            let arg_types: Vec<String> = args.iter().map(|a| a.type_.clone()).collect();
            let signature = format!("\"{}\"({})", row.name, arg_types.join(", "));

            functions.insert(
                signature,
                FunctionInfo {
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

    async fn get_views(&self) -> Result<HashMap<String, ViewInfo>, String> {
        let query = r#"
            WITH view_data AS (
                -- Regular views
                SELECT
                    c.relname as name,
                    pg_get_viewdef(c.oid, true) as definition,
                    false as is_materialized,
                    obj_description(c.oid, 'pg_class') as comment,
                    c.reloptions as options,
                    c.oid
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = 'public'
                AND c.relkind = 'v'

                UNION ALL

                -- Materialized views
                SELECT
                    c.relname as name,
                    pg_get_viewdef(c.oid, true) as definition,
                    true as is_materialized,
                    obj_description(c.oid, 'pg_class') as comment,
                    c.reloptions as options,
                    c.oid
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = 'public'
                AND c.relkind = 'm'
            ),
            view_columns AS (
                SELECT
                    c.relname as view_name,
                    a.attname as column_name,
                    format_type(a.atttypid, a.atttypmod) as data_type,
                    col_description(c.oid, a.attnum) as comment
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                JOIN pg_attribute a ON a.attrelid = c.oid
                WHERE n.nspname = 'public'
                AND c.relkind IN ('v', 'm')
                AND a.attnum > 0
                AND NOT a.attisdropped
            ),
            mat_view_indexes AS (
                SELECT
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
                WHERE n.nspname = 'public'
                AND t.relkind = 'm'
                GROUP BY t.relname, i.relname, ix.indisunique, am.amname, ix.indpred, ix.indrelid
            )
            SELECT json_build_object(
                'views', (SELECT json_agg(row_to_json(view_data)) FROM view_data),
                'columns', (SELECT json_agg(row_to_json(view_columns)) FROM view_columns),
                'indexes', (SELECT json_agg(row_to_json(mat_view_indexes)) FROM mat_view_indexes)
            ) as data
        "#;

        let result = self
            .api
            .run_query(&self.project_ref, query, true)
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

        self.parse_views_response(&data)
    }

    fn parse_views_response(
        &self,
        data: &serde_json::Value,
    ) -> Result<HashMap<String, ViewInfo>, String> {
        #[derive(Deserialize)]
        struct ViewRow {
            name: String,
            definition: Option<String>,
            is_materialized: bool,
            comment: Option<String>,
            options: Option<serde_json::Value>,
        }

        #[derive(Deserialize)]
        struct ColumnRow {
            view_name: String,
            column_name: String,
            data_type: String,
            comment: Option<String>,
        }

        #[derive(Deserialize)]
        struct IndexRow {
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

            views.insert(
                row.name.clone(),
                ViewInfo {
                    name: row.name,
                    definition: row.definition.unwrap_or_default(),
                    is_materialized: row.is_materialized,
                    columns: vec![],
                    indexes: vec![],
                    comment: row.comment,
                    with_options: options,
                    check_option: None,
                },
            );
        }

        // Add columns to views
        for col in column_rows {
            if let Some(view) = views.get_mut(&col.view_name) {
                view.columns.push(ViewColumnInfo {
                    name: col.column_name,
                    data_type: col.data_type,
                    comment: col.comment,
                });
            }
        }

        // Add indexes to materialized views
        for idx in index_rows {
            if let Some(view) = views.get_mut(&idx.view_name) {
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

    async fn get_sequences(&self) -> Result<HashMap<String, SequenceInfo>, String> {
        let query = r#"
            SELECT
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
            WHERE n.nspname = 'public'
            AND s.relkind = 'S'
        "#;

        #[derive(Deserialize)]
        struct Row {
            name: String,
            data_type: String,
            start_value: i64,
            min_value: i64,
            max_value: i64,
            increment: i64,
            cycle: bool,
            cache_size: i64,
            owned_by: Option<String>,
            comment: Option<String>,
        }

        let result = self
            .api
            .run_query(&self.project_ref, query, true)
            .await
            .map_err(|e| e.to_string())?;

        let rows: Vec<Row> =
            serde_json::from_value(result.result.unwrap_or(serde_json::Value::Array(vec![])))
                .map_err(|e| e.to_string())?;

        let mut sequences = HashMap::new();
        for row in rows {
            sequences.insert(
                row.name.clone(),
                SequenceInfo {
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

    async fn get_extensions(&self) -> Result<HashMap<String, ExtensionInfo>, String> {
        let query = r#"
            SELECT
                e.extname as name,
                e.extversion as version,
                n.nspname as schema
            FROM pg_extension e
            JOIN pg_namespace n ON n.oid = e.extnamespace
            WHERE e.extname != 'plpgsql'  -- Skip built-in
        "#;

        #[derive(Deserialize)]
        struct Row {
            name: String,
            version: Option<String>,
            schema: Option<String>,
        }

        let result = self
            .api
            .run_query(&self.project_ref, query, true)
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

    async fn get_composite_types(&self) -> Result<HashMap<String, CompositeTypeInfo>, String> {
        let query = r#"
            SELECT
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
            WHERE n.nspname = 'public'
            AND t.typtype = 'c'
            AND cls.relkind = 'c'
            GROUP BY t.typname, t.oid
        "#;

        #[derive(Deserialize)]
        struct Row {
            name: String,
            attributes: serde_json::Value,
            comment: Option<String>,
        }

        let result = self
            .api
            .run_query(&self.project_ref, query, true)
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

            types.insert(
                row.name.clone(),
                CompositeTypeInfo {
                    name: row.name,
                    attributes: attrs,
                    comment: row.comment,
                },
            );
        }

        Ok(types)
    }

    async fn get_domains(&self) -> Result<HashMap<String, DomainInfo>, String> {
        let query = r#"
            SELECT
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
            WHERE n.nspname = 'public'
            AND t.typtype = 'd'
        "#;

        #[derive(Deserialize)]
        struct Row {
            name: String,
            base_type: String,
            default_value: Option<String>,
            is_not_null: bool,
            collation: Option<String>,
            comment: Option<String>,
            check_constraints: Option<serde_json::Value>,
        }

        let result = self
            .api
            .run_query(&self.project_ref, query, true)
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

            domains.insert(
                row.name.clone(),
                DomainInfo {
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

    /// Fetch all table information using bulk queries (minimal API calls)
    async fn get_all_tables_bulk(&self) -> Result<HashMap<String, TableInfo>, String> {
        // Single comprehensive query that gets tables + columns + constraints
        let bulk_query = r#"
            WITH table_list AS (
                SELECT table_name
                FROM information_schema.tables
                WHERE table_schema = 'public'
                AND table_type = 'BASE TABLE'
            ),
            columns_data AS (
                SELECT
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
                WHERE n.nspname = 'public'
                AND t.relkind = 'r'
                AND a.attnum > 0
                AND NOT a.attisdropped
            ),
            fk_data AS (
                SELECT
                    c.relname as table_name,
                    con.conname as constraint_name,
                    a.attname as column_name,
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
                CROSS JOIN LATERAL unnest(con.conkey, con.confkey) AS k(con_attnum, conf_attnum)
                JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = k.con_attnum
                JOIN pg_attribute af ON af.attrelid = cf.oid AND af.attnum = k.conf_attnum
                WHERE n.nspname = 'public'
                AND con.contype = 'f'
            ),
            index_data AS (
                SELECT
                    t.relname as table_name,
                    i.relname as index_name,
                    array_agg(a.attname ORDER BY array_position(ix.indkey, a.attnum)) as columns,
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
                JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
                JOIN pg_namespace n ON t.relnamespace = n.oid
                LEFT JOIN pg_constraint con ON con.conindid = i.oid
                WHERE n.nspname = 'public'
                AND NOT ix.indisprimary
                GROUP BY t.relname, i.relname, ix.indisunique, ix.indisprimary, am.amname, ix.indpred, ix.indrelid, i.oid
            ),
            trigger_data AS (
                SELECT
                    c.relname as table_name,
                    t.tgname as trigger_name,
                    t.tgtype::integer as tgtype,
                    p.proname as function_name,
                    pg_get_triggerdef(t.oid) as trigger_def
                FROM pg_trigger t
                JOIN pg_class c ON c.oid = t.tgrelid
                JOIN pg_namespace n ON n.oid = c.relnamespace
                JOIN pg_proc p ON p.oid = t.tgfoid
                WHERE n.nspname = 'public'
                AND NOT t.tgisinternal
            ),
            policy_data AS (
                SELECT
                    c.relname as table_name,
                    p.polname as name,
                    p.polcmd as cmd,
                    p.polroles as roles,
                    pg_get_expr(p.polqual, p.polrelid) as qual,
                    pg_get_expr(p.polwithcheck, p.polrelid) as with_check
                FROM pg_policy p
                JOIN pg_class c ON c.oid = p.polrelid
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = 'public'
            ),
            rls_data AS (
                SELECT c.relname as table_name, c.relrowsecurity as rls_enabled
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = 'public' AND c.relkind = 'r'
            ),
            check_data AS (
                SELECT
                    c.relname as table_name,
                    con.conname as name,
                    pg_get_constraintdef(con.oid) as expression,
                    array_agg(a.attname ORDER BY a.attnum) as columns
                FROM pg_constraint con
                JOIN pg_class c ON con.conrelid = c.oid
                JOIN pg_namespace n ON c.relnamespace = n.oid
                LEFT JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum = ANY(con.conkey)
                WHERE n.nspname = 'public'
                AND con.contype = 'c'
                GROUP BY c.relname, con.conname, con.oid
            ),
            table_comments AS (
                SELECT
                    c.relname as table_name,
                    obj_description(c.oid, 'pg_class') as comment
                FROM pg_class c
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE n.nspname = 'public'
                AND c.relkind = 'r'
            )
            SELECT json_build_object(
                'tables', (SELECT json_agg(table_name) FROM table_list),
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

        let result = self
            .api
            .run_query(&self.project_ref, bulk_query, true)
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

        // Parse the bulk response
        self.parse_bulk_response(&data)
    }

    async fn get_roles(&self) -> Result<HashMap<String, RoleInfo>, String> {
        // Query to get roles, filtering out system roles
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

        let result = self
            .api
            .run_query(&self.project_ref, query, true)
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
                    password: None, // We don't fetch passwords for security
                },
            );
        }

        Ok(roles)
    }

    fn parse_bulk_response(
        &self,
        data: &serde_json::Value,
    ) -> Result<HashMap<String, TableInfo>, String> {
        // Extract table names
        let table_names: Vec<String> = data
            .get("tables")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Parse columns
        #[derive(Deserialize)]
        struct ColumnRow {
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
            comment: Option<String>,
        }
        let columns: Vec<ColumnRow> = data
            .get("columns")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        // Parse foreign keys
        #[derive(Deserialize)]
        struct FkRow {
            table_name: String,
            constraint_name: String,
            column_name: String,
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

        // Parse indexes
        #[derive(Deserialize)]
        struct IndexRow {
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

        // Parse triggers
        #[derive(Deserialize)]
        struct TriggerRow {
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

        // Parse policies
        #[derive(Deserialize)]
        struct PolicyRow {
            table_name: String,
            name: String,
            cmd: String,
            roles: serde_json::Value,
            qual: Option<String>,
            with_check: Option<String>,
        }
        let policies: Vec<PolicyRow> = data
            .get("policies")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        // Parse RLS
        #[derive(Deserialize)]
        struct RlsRow {
            table_name: String,
            rls_enabled: bool,
        }
        let rls_data: Vec<RlsRow> = data
            .get("rls")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        // Parse check constraints
        #[derive(Deserialize)]
        struct CheckRow {
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

        // Parse table comments
        #[derive(Deserialize)]
        struct CommentRow {
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
        for table_name in table_names {
            tables.insert(
                table_name.clone(),
                TableInfo {
                    table_name,
                    columns: HashMap::new(),
                    foreign_keys: vec![],
                    indexes: vec![],
                    triggers: vec![],
                    rls_enabled: false,
                    policies: vec![],
                    check_constraints: vec![],
                    comment: None,
                },
            );
        }

        // Populate columns
        for col in columns {
            if let Some(table) = tables.get_mut(&col.table_name) {
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
                        data_type: final_data_type,
                        is_nullable: col.is_nullable == "YES",
                        column_default: col.column_default,
                        udt_name: col.udt_name.clone(),
                        is_primary_key: col.is_primary_key,
                        is_unique: col.is_unique,
                        is_identity: col.is_identity == "YES",
                        identity_generation: col.identity_generation,
                        collation: col.collation,
                        enum_name: None,
                        is_array: col.udt_name.starts_with('_'),
                        comment: col.comment,
                    },
                );
            }
        }

        // Populate foreign keys
        for fk in fks {
            if let Some(table) = tables.get_mut(&fk.table_name) {
                table.foreign_keys.push(ForeignKeyInfo {
                    constraint_name: fk.constraint_name,
                    column_name: fk.column_name,
                    foreign_table: fk.foreign_table,
                    foreign_column: fk.foreign_column,
                    on_delete: fk.on_delete,
                    on_update: fk.on_update,
                });
            }
        }

        // Populate indexes
        println!("[DEBUG] Indexes fetched from DB: {}", indexes.len());
        for idx in indexes {
            println!(
                "[DEBUG] Adding index {} to table {}, columns: {:?}",
                idx.index_name, idx.table_name, idx.columns
            );
            if let Some(table) = tables.get_mut(&idx.table_name) {
                // Extract expressions from index_def if present
                let expressions = idx
                    .index_def
                    .as_ref()
                    .map(|def| extract_index_expressions(def))
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

        // Populate triggers (consolidate by trigger name)
        let mut trigger_map: HashMap<(String, String), TriggerInfo> = HashMap::new();
        for trig in triggers {
            let key = (trig.table_name.clone(), trig.trigger_name.clone());

            // Extract WHEN clause from trigger definition
            let when_clause = trig
                .trigger_def
                .as_ref()
                .and_then(|def| extract_trigger_when_clause(def));

            let entry = trigger_map.entry(key).or_insert_with(|| {
                // Decode tgtype for timing and orientation (once)
                let timing = if trig.tgtype & 2 != 0 {
                    "BEFORE"
                } else if trig.tgtype & 64 != 0 {
                    "INSTEAD OF"
                } else {
                    "AFTER"
                };
                let orientation = if trig.tgtype & 1 != 0 {
                    "ROW"
                } else {
                    "STATEMENT"
                };

                TriggerInfo {
                    name: trig.trigger_name.clone(),
                    events: vec![],
                    timing: timing.to_string(),
                    orientation: orientation.to_string(),
                    function_name: trig.function_name.clone(),
                    when_clause,
                }
            });

            // Decode tgtype for events (bitmask)
            if trig.tgtype & 4 != 0 && !entry.events.contains(&"INSERT".to_string()) {
                entry.events.push("INSERT".to_string());
            }
            if trig.tgtype & 8 != 0 && !entry.events.contains(&"DELETE".to_string()) {
                entry.events.push("DELETE".to_string());
            }
            if trig.tgtype & 16 != 0 && !entry.events.contains(&"UPDATE".to_string()) {
                entry.events.push("UPDATE".to_string());
            }
            if trig.tgtype & 32 != 0 && !entry.events.contains(&"TRUNCATE".to_string()) {
                entry.events.push("TRUNCATE".to_string());
            }
        }

        for ((table_name, _), trigger) in trigger_map {
            if let Some(table) = tables.get_mut(&table_name) {
                table.triggers.push(trigger);
            }
        }

        // Populate policies
        for pol in policies {
            if let Some(table) = tables.get_mut(&pol.table_name) {
                table.policies.push(PolicyInfo {
                    name: pol.name,
                    cmd: parse_policy_cmd(&pol.cmd),
                    roles: parse_pg_oid_array(&pol.roles),
                    qual: pol.qual,
                    with_check: pol.with_check,
                });
            }
        }

        // Populate RLS status
        for rls in rls_data {
            if let Some(table) = tables.get_mut(&rls.table_name) {
                table.rls_enabled = rls.rls_enabled;
            }
        }

        // Populate check constraints
        for check in check_data {
            if let Some(table) = tables.get_mut(&check.table_name) {
                table.check_constraints.push(CheckConstraintInfo {
                    name: check.name,
                    expression: check.expression,
                    columns: parse_pg_array(&check.columns),
                });
            }
        }

        // Populate table comments
        for comment in comment_data {
            if let Some(table) = tables.get_mut(&comment.table_name) {
                table.comment = comment.comment;
            }
        }

        Ok(tables)
    }
}

// Helpers

fn parse_pg_array(val: &serde_json::Value) -> Vec<String> {
    if let Some(arr) = val.as_array() {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    } else if let Some(s) = val.as_str() {
        // Handle "{a,b}" string
        s.trim_matches(|c| c == '{' || c == '}')
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        vec![]
    }
}

fn parse_pg_oid_array(val: &serde_json::Value) -> Vec<String> {
    // Simplified: return "public" if {0} or equivalent, else "authenticated" placeholder
    let s = val.to_string();
    if s.contains("{0}") {
        vec!["public".to_string()]
    } else {
        vec!["authenticated".to_string()] // Placeholder until we map OIDs
    }
}

fn parse_policy_cmd(cmd: &str) -> String {
    match cmd {
        "r" => "SELECT".to_string(),
        "a" => "INSERT".to_string(),
        "w" => "UPDATE".to_string(),
        "d" => "DELETE".to_string(),
        "*" => "ALL".to_string(),
        _ => cmd.to_string(),
    }
}

fn parse_function_args(args_str: &str) -> Vec<FunctionArg> {
    if args_str.is_empty() {
        return vec![];
    }
    args_str
        .split(',')
        .map(|s| {
            let trimmed = s.trim();
            let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();

            // Check for mode keywords
            let (mode, name_type) = if parts.first().map(|p| p.to_uppercase()).as_deref()
                == Some("IN")
                || parts.first().map(|p| p.to_uppercase()).as_deref() == Some("OUT")
                || parts.first().map(|p| p.to_uppercase()).as_deref() == Some("INOUT")
                || parts.first().map(|p| p.to_uppercase()).as_deref() == Some("VARIADIC")
            {
                (
                    Some(parts[0].to_uppercase()),
                    parts.get(1).map(|s| *s).unwrap_or(""),
                )
            } else {
                (None, trimmed)
            };

            let name_type_parts: Vec<&str> = name_type.splitn(2, ' ').collect();
            let (name, type_str) = if name_type_parts.len() >= 2 {
                (
                    name_type_parts[0].to_string(),
                    name_type_parts[1..].join(" "),
                )
            } else {
                (String::new(), name_type.to_string())
            };

            // Check for DEFAULT
            let (final_type, default_value) =
                if let Some(idx) = type_str.to_uppercase().find(" DEFAULT ") {
                    (
                        type_str[..idx].to_string(),
                        Some(type_str[idx + 9..].to_string()),
                    )
                } else {
                    (type_str, None)
                };

            FunctionArg {
                name,
                type_: final_type,
                mode,
                default_value,
            }
        })
        .collect()
}

fn extract_trigger_when_clause(trigger_def: &str) -> Option<String> {
    // Look for WHEN (...) in the trigger definition
    let upper = trigger_def.to_uppercase();
    if let Some(when_idx) = upper.find(" WHEN ") {
        let after_when = &trigger_def[when_idx + 6..];
        // Find matching parentheses
        if let Some(start) = after_when.find('(') {
            let mut depth = 0;
            let mut end = None;
            for (i, c) in after_when[start..].char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(start + i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(e) = end {
                return Some(after_when[start + 1..e].to_string());
            }
        }
    }
    None
}

fn extract_index_expressions(index_def: &str) -> Vec<String> {
    // Extract expressions from index definition like "CREATE INDEX ... ON table ((lower(email)))"
    // This is a simplified extraction - looks for expressions in double parens or function calls
    let mut expressions = vec![];

    if let Some(on_idx) = index_def.to_uppercase().find(" ON ") {
        let after_on = &index_def[on_idx + 4..];
        if let Some(paren_start) = after_on.find('(') {
            let in_parens = &after_on[paren_start + 1..];
            if let Some(paren_end) = in_parens.rfind(')') {
                let cols_str = &in_parens[..paren_end];
                // Check for expressions (contain parentheses themselves)
                for part in cols_str.split(',') {
                    let trimmed = part.trim();
                    if trimmed.contains('(') {
                        expressions.push(trimmed.to_string());
                    }
                }
            }
        }
    }

    expressions
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_bulk_response_array_type() {
        let api = SupabaseApi::new("token".to_string());
        let introspector = Introspector::new(&api, "project".to_string());

        let data = json!({
            "tables": ["test_table"],
            "columns": [
                {
                    "table_name": "test_table",
                    "column_name": "tags",
                    "data_type": "ARRAY",
                    "is_nullable": "NO",
                    "column_default": null,
                    "udt_name": "_text",
                    "is_identity": "NO",
                    "is_primary_key": false,
                    "is_unique": false,
                    "comment": null
                }
            ],
            "foreign_keys": [],
            "indexes": [],
            "triggers": [],
            "policies": [],
            "rls": [],
            "check_constraints": [],
            "table_comments": []
        });

        let result = introspector.parse_bulk_response(&data).unwrap();
        let table = result.get("test_table").unwrap();
        let col = table.columns.get("tags").unwrap();

        assert_eq!(col.data_type, "text[]");
    }

    #[test]
    fn test_parse_bulk_response_pg_trigger() {
        let api = SupabaseApi::new("token".to_string());
        let introspector = Introspector::new(&api, "project".to_string());

        let data = json!({
            "tables": ["test_table"],
            "columns": [],
            "foreign_keys": [],
            "indexes": [],
            "triggers": [
                {
                    "table_name": "test_table",
                    "trigger_name": "test_trigger",
                    "tgtype": 21,
                    "function_name": "test_func",
                    "trigger_def": "CREATE TRIGGER test_trigger AFTER INSERT OR UPDATE ON test_table FOR EACH ROW EXECUTE FUNCTION test_func()"
                }
            ],
            "policies": [],
            "rls": [],
            "check_constraints": [],
            "table_comments": []
        });

        let result = introspector.parse_bulk_response(&data).unwrap();
        let table = result.get("test_table").unwrap();
        let trigger = &table.triggers[0];

        assert_eq!(trigger.name, "test_trigger");
        assert_eq!(trigger.orientation, "ROW");
        assert_eq!(trigger.timing, "AFTER");
        assert!(trigger.events.contains(&"UPDATE".to_string()));
        assert!(trigger.events.contains(&"INSERT".to_string()));
        assert_eq!(trigger.function_name, "test_func");
    }

    #[test]
    fn test_parse_bulk_response_with_check_constraints() {
        let api = SupabaseApi::new("token".to_string());
        let introspector = Introspector::new(&api, "project".to_string());

        let data = json!({
            "tables": ["users"],
            "columns": [],
            "foreign_keys": [],
            "indexes": [],
            "triggers": [],
            "policies": [],
            "rls": [],
            "check_constraints": [
                {
                    "table_name": "users",
                    "name": "age_check",
                    "expression": "CHECK ((age > 0))",
                    "columns": ["age"]
                }
            ],
            "table_comments": []
        });

        let result = introspector.parse_bulk_response(&data).unwrap();
        let table = result.get("users").unwrap();
        assert_eq!(table.check_constraints.len(), 1);
        assert_eq!(table.check_constraints[0].name, "age_check");
    }

    #[test]
    fn test_extract_trigger_when_clause() {
        let def = "CREATE TRIGGER my_trigger AFTER UPDATE ON users FOR EACH ROW WHEN (OLD.status IS DISTINCT FROM NEW.status) EXECUTE FUNCTION notify()";
        let when = extract_trigger_when_clause(def);
        assert_eq!(
            when,
            Some("OLD.status IS DISTINCT FROM NEW.status".to_string())
        );
    }

    #[test]
    fn test_parse_function_args_with_defaults() {
        let args = parse_function_args("name text, age integer DEFAULT 0, OUT result text");
        assert_eq!(args.len(), 3);
        assert_eq!(args[0].name, "name");
        assert_eq!(args[0].type_, "text");
        assert_eq!(args[1].name, "age");
        assert_eq!(args[1].type_, "integer");
        assert_eq!(args[1].default_value, Some("0".to_string()));
        assert_eq!(args[2].mode, Some("OUT".to_string()));
    }
}
