mod functions;
mod helpers;
mod queries;
mod roles;
mod sequences;
pub mod tables;
mod types;
mod views;

use helpers::*;

use crate::schema::{
    CompositeTypeInfo, DbSchema, DomainInfo, EnumInfo, ExtensionInfo,
    FunctionInfo, RoleInfo, SequenceInfo, TableInfo, ViewInfo,
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
        types::get_enums(self.api, &self.project_ref).await
    }

    async fn get_functions(&self) -> Result<HashMap<String, FunctionInfo>, String> {
        functions::get_functions(self.api, &self.project_ref).await
    }

    async fn get_views(&self) -> Result<HashMap<String, ViewInfo>, String> {
        views::get_views(self.api, &self.project_ref).await
    }

    async fn get_sequences(&self) -> Result<HashMap<String, SequenceInfo>, String> {
        sequences::get_sequences(self.api, &self.project_ref).await
    }

    async fn get_extensions(&self) -> Result<HashMap<String, ExtensionInfo>, String> {
        roles::get_extensions(self.api, &self.project_ref).await
    }

    async fn get_composite_types(&self) -> Result<HashMap<String, CompositeTypeInfo>, String> {
        types::get_composite_types(self.api, &self.project_ref).await
    }

    async fn get_domains(&self) -> Result<HashMap<String, DomainInfo>, String> {
        types::get_domains(self.api, &self.project_ref).await
    }

    /// Fetch all table information using bulk queries (minimal API calls)
    async fn get_all_tables_bulk(&self) -> Result<HashMap<String, TableInfo>, String> {
        tables::get_all_tables_bulk(self.api, &self.project_ref).await
    }

    async fn get_roles(&self) -> Result<HashMap<String, RoleInfo>, String> {
        roles::get_roles(self.api, &self.project_ref).await
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_bulk_response_array_type() {
        let api = SupabaseApi::new("token".to_string());
        let _introspector = Introspector::new(&api, "project".to_string());

        let data = json!({
            "tables": [{"schema": "public", "name": "test_table"}],
            "columns": [
                {
                    "schema": "public",
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

        let result = tables::parse_bulk_response(&data).unwrap();
        let table = result.get("\"public\".\"test_table\"").unwrap();
        let col = table.columns.get("tags").unwrap();

        assert_eq!(col.data_type, "text[]");
    }

    #[test]
    fn test_parse_bulk_response_pg_trigger() {
        let api = SupabaseApi::new("token".to_string());
        let _introspector = Introspector::new(&api, "project".to_string());

        let data = json!({
            "tables": [{"schema": "public", "name": "test_table"}],
            "columns": [],
            "foreign_keys": [],
            "indexes": [],
            "triggers": [
                {
                    "schema": "public",
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

        let result = tables::parse_bulk_response(&data).unwrap();
        let table = result.get("\"public\".\"test_table\"").unwrap();
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
        let _introspector = Introspector::new(&api, "project".to_string());

        let data = json!({
            "tables": [{"schema": "public", "name": "users"}],
            "columns": [],
            "foreign_keys": [],
            "indexes": [],
            "triggers": [],
            "policies": [],
            "rls": [],
            "check_constraints": [
                {
                    "schema": "public",
                    "table_name": "users",
                    "name": "age_check",
                    "expression": "CHECK ((age > 0))",
                    "columns": ["age"]
                }
            ],
            "table_comments": []
        });

        let result = tables::parse_bulk_response(&data).unwrap();
        let table = result.get("\"public\".\"users\"").unwrap();
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

    #[test]
    fn test_deserialize_large_sequence_value() {
        #[derive(Deserialize, Debug)]
        struct Row {
            #[serde(deserialize_with = "deserialize_i64_or_string")]
            val: i64,
        }

        let json_str = r#"{"val": "9223372036854775807"}"#;
        let row: Row = serde_json::from_str(json_str).expect("Failed to parse stringified i64");
        assert_eq!(row.val, i64::MAX);

        let json_int = r#"{"val": 123}"#;
        let row_int: Row = serde_json::from_str(json_int).expect("Failed to parse int i64");
        assert_eq!(row_int.val, 123);
    }
}
