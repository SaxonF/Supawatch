use crate::schema::DbSchema;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sqlparser::ast::Statement;
use std::collections::HashMap;

mod constraints;
mod functions;
mod helpers;
mod roles;
mod sequences;
mod tables;
mod types;
mod views;

pub fn parse_schema_sql(sql: &str) -> Result<DbSchema, String> {
    // SECURITY DEFINER workaround:
    // sqlparser-rs doesn't support SECURITY DEFINER yet, so we manually extract it
    // and remove it from the SQL before parsing.
    let (cleaned_sql, security_definer_funcs) = preprocess_security_definer(sql);

    let dialect = PostgreSqlDialect {};
    let ast = Parser::parse_sql(&dialect, &cleaned_sql).map_err(|e| e.to_string())?;

    let mut tables = HashMap::new();
    let mut enums = HashMap::new();
    let mut functions = HashMap::new();
    let mut roles = HashMap::new();
    let mut views = HashMap::new();
    let mut sequences = HashMap::new();
    let mut extensions = HashMap::new();
    let mut composite_types = HashMap::new();
    let mut domains = HashMap::new();

    for statement in ast {
        match statement {
            Statement::CreateTable(stmt) => {
                tables::handle_create_table(&mut tables, stmt);
            }
            Statement::CreateType {
                name,
                representation,
                ..
            } => {
                types::handle_create_type(&mut enums, &mut composite_types, name, representation);
            }
            Statement::CreateFunction(stmt) => {
                let (schema, name) = helpers::parse_object_name(&stmt.name);
                // Check multiple formats for the function name as it appeared in SQL
                let is_sec_def = security_definer_funcs.contains(&format!("\"{}\".\"{}\"", schema, name)) ||
                                 security_definer_funcs.contains(&format!("{}.{}", schema, name)) ||
                                 security_definer_funcs.contains(&name);
                
                functions::handle_create_function(&mut functions, stmt, is_sec_def);
            }
            Statement::CreateRole(stmt) => {
                roles::handle_create_role(&mut roles, stmt);
            }
            Statement::CreateTrigger(stmt) => {
                constraints::handle_create_trigger(&mut tables, stmt);
            }
            Statement::CreatePolicy {
                name,
                table_name,
                command,
                to,
                using,
                with_check,
                ..
            } => {
                constraints::handle_create_policy(
                    &mut tables,
                    name,
                    table_name,
                    command,
                    to,
                    using,
                    with_check,
                );
            }
            Statement::AlterTable(stmt) => {
                tables::handle_alter_table(&mut tables, stmt);
            }
            Statement::CreateIndex(stmt) => {
                tables::handle_create_index(&mut tables, stmt);
            }
            Statement::CreateView(stmt) => {
                views::handle_create_view(&mut views, stmt);
            }
            Statement::CreateSequence {
                name,
                data_type,
                sequence_options,
                ..
            } => {
                sequences::handle_create_sequence(&mut sequences, name, data_type, sequence_options);
            }
            Statement::CreateExtension(stmt) => {
                roles::handle_create_extension(&mut extensions, stmt);
            }
            Statement::CreateDomain(stmt) => {
                types::handle_create_domain(&mut domains, stmt);
            }
            Statement::Comment {
                object_type,
                object_name,
                comment,
                ..
            } => {
                tables::handle_comment(&mut tables, object_type, object_name, comment);
            }
            _ => {}
        }
    }

    Ok(DbSchema {
        tables,
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

fn preprocess_security_definer(sql: &str) -> (String, std::collections::HashSet<String>) {
    let mut cleaned_sql = sql.to_string();
    let mut sec_def_funcs = std::collections::HashSet::new();
    
    // Simple case-insensitive match for SECURITY DEFINER
    // We assume it's correctly placed in a CREATE FUNCTION statement
    let key = "SECURITY DEFINER";
    
    // We iterate backwards to find matches and replace them, to preserve indices
    // Actually simple replace is easier for cleaning, but we need to extract names first.
    
    // We scan the original SQL for names
    let sql_upper = sql.to_uppercase();
    let mut start_search = 0;
    
    while let Some(idx) = sql_upper[start_search..].find(key) {
        let abs_idx = start_search + idx;
        
        // Find preceding "FUNCTION"
        if let Some(func_idx) = sql_upper[..abs_idx].rfind("FUNCTION") {
             // Find the opening parenthesis after FUNCTION to isolate name
             if let Some(paren_idx) = sql[func_idx..abs_idx].find('(') {
                 let raw_name = sql[func_idx + 8 .. func_idx + paren_idx].trim();
                 sec_def_funcs.insert(raw_name.to_string());
             }
        }
        
        start_search = abs_idx + key.len();
    }
    
    // Now remove the clauses
    cleaned_sql = cleaned_sql.replace("SECURITY DEFINER", "");
    
    (cleaned_sql, sec_def_funcs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_schema() {
        let sql = r#"
CREATE OR REPLACE FUNCTION update_player_last_played() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RETURN NEW;
END;
$$;

CREATE TABLE players (
    id uuid NOT NULL
);

CREATE TRIGGER update_player_timestamp BEFORE UPDATE ON players FOR EACH ROW EXECUTE FUNCTION update_player_last_played();

CREATE POLICY "public_read" ON players FOR SELECT USING (true);

ALTER TABLE players ENABLE ROW LEVEL SECURITY;
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        // Verify Function
        let func = schema
            .functions
            .get("\"public\".\"update_player_last_played\"()")
            .expect("Function not found");
        assert_eq!(func.language, "plpgsql");
        assert_eq!(func.return_type, "trigger");

        // Verify Table
        let table = schema.tables.get("\"public\".\"players\"").expect("Table not found");
        assert!(table.rls_enabled);

        // Verify Trigger
        assert_eq!(table.triggers.len(), 1);
        let trigger = &table.triggers[0];
        assert_eq!(trigger.name, "update_player_timestamp");
        assert_eq!(trigger.timing, "BEFORE");
        assert_eq!(trigger.orientation, "ROW");
        assert_eq!(trigger.function_name, "update_player_last_played");

        // Verify Policy
        assert_eq!(table.policies.len(), 1);
        let policy = &table.policies[0];
        assert_eq!(policy.name, "public_read");
        assert_eq!(policy.cmd, "SELECT");
    }

    #[test]
    fn test_parse_schema_mismatch() {
        let sql = r#"
CREATE TABLE players (
    id uuid NOT NULL
);

CREATE TRIGGER update_player_timestamp BEFORE UPDATE ON public.players FOR EACH ROW EXECUTE FUNCTION update_player_last_played();
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        // Verify Table
        let table = schema.tables.get("\"public\".\"players\"").expect("Table not found");

        // Verify Trigger should exist even if ON public.players
        assert_eq!(table.triggers.len(), 1);
        let trigger = &table.triggers[0];
        assert_eq!(trigger.name, "update_player_timestamp");
    }

    #[test]
    fn test_parse_views() {
        let sql = r#"
CREATE VIEW user_stats AS SELECT id, count(*) as post_count FROM users GROUP BY id;
CREATE MATERIALIZED VIEW cached_stats AS SELECT * FROM user_stats;
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        assert_eq!(schema.views.len(), 2);

        let view = schema.views.get("\"public\".\"user_stats\"").expect("View not found");
        assert!(!view.is_materialized);

        let mat_view = schema
            .views
            .get("\"public\".\"cached_stats\"")
            .expect("Materialized view not found");
        assert!(mat_view.is_materialized);
    }

    #[test]
    fn test_parse_sequences() {
        let sql = r#"
CREATE SEQUENCE user_id_seq INCREMENT BY 1 MINVALUE 1 MAXVALUE 1000000 CACHE 10;
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let seq = schema
            .sequences
            .get("\"public\".\"user_id_seq\"")
            .expect("Sequence not found");
        assert_eq!(seq.increment, 1);
        assert_eq!(seq.min_value, 1);
        assert_eq!(seq.max_value, 1000000);
        assert_eq!(seq.cache_size, 10);
    }

    #[test]
    fn test_parse_composite_types() {
        let sql = r#"
CREATE TYPE address AS (
    street text,
    city text,
    zip_code text
);
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let addr_type = schema
            .composite_types
            .get("\"public\".\"address\"")
            .expect("Composite type not found");
        assert_eq!(addr_type.attributes.len(), 3);
        assert_eq!(addr_type.attributes[0].name, "street");
    }

    #[test]
    fn test_parse_check_constraints() {
        let sql = r#"
CREATE TABLE users (
    id uuid NOT NULL,
    age integer CHECK (age > 0),
    CONSTRAINT valid_age CHECK (age < 150)
);
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");

        assert!(table.check_constraints.len() >= 1);
    }

    #[test]
    fn test_parse_partial_index() {
        let sql = r#"
CREATE TABLE users (id uuid NOT NULL, active boolean);
CREATE INDEX active_users_idx ON users (id) WHERE active = true;
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");

        let idx = table
            .indexes
            .iter()
            .find(|i| i.index_name == "active_users_idx")
            .expect("Index not found");
        assert!(idx.where_clause.is_some());
    }

    #[test]
    fn test_parse_indexes_and_constraints() {
        let sql = r#"
CREATE TABLE users ( id uuid );
CREATE UNIQUE INDEX idx_email ON users (email);
ALTER TABLE users ADD CONSTRAINT fk_role FOREIGN KEY (role_id) REFERENCES roles(id);
ALTER TABLE users ADD CONSTRAINT unique_username UNIQUE (username);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");

        // Verify CREATE INDEX
        assert!(table
            .indexes
            .iter()
            .any(|i| i.index_name == "idx_email" && i.is_unique));

        // Verify ALTER TABLE FK
        assert!(table
            .foreign_keys
            .iter()
            .any(|fk| fk.constraint_name == "fk_role"));

        // Verify ALTER TABLE UNIQUE (should be an index with constraint)
        assert!(table
            .indexes
            .iter()
            .any(|i| i.index_name == "unique_username"
                && i.owning_constraint.as_deref() == Some("unique_username")));
    }

    #[test]
    fn test_parse_identity_and_collation() {
        let sql = r#"
CREATE TABLE items (
    id integer GENERATED ALWAYS AS IDENTITY,
    code text COLLATE "C"
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"items\"").expect("Table not found");

        let id_col = table.columns.get("id").expect("id column not found");
        assert!(id_col.is_identity);
        assert_eq!(id_col.identity_generation, Some("ALWAYS".to_string()));

        let code_col = table.columns.get("code").expect("code column not found");
        assert_eq!(code_col.collation, Some("\"C\"".to_string()));
    }

    #[test]
    fn test_parse_function_overloading() {
        let sql = r#"
CREATE FUNCTION add(a integer, b integer) RETURNS integer LANGUAGE sql AS 'SELECT a + b';
CREATE FUNCTION add(a float, b float) RETURNS float LANGUAGE sql AS 'SELECT a + b';
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        assert_eq!(schema.functions.len(), 2);
        assert!(schema.functions.contains_key("\"public\".\"add\"(integer, integer)"));
        assert!(schema.functions.contains_key("\"public\".\"add\"(float, float)"));
    }

    #[test]
    fn test_parse_roles() {
        let sql = r#"
CREATE ROLE "Test" WITH LOGIN SUPERUSER PASSWORD 'secret';
CREATE ROLE "readonly";
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        assert!(schema.roles.contains_key("Test"));
        let test_role = schema.roles.get("Test").unwrap();
        assert!(test_role.login);
        assert!(test_role.superuser);
        assert_eq!(test_role.password, Some("secret".to_string()));

        assert!(schema.roles.contains_key("readonly"));
        let readonly_role = schema.roles.get("readonly").unwrap();
        assert!(!readonly_role.superuser);
        assert!(!readonly_role.login);
    }

    #[test]
    fn test_parse_extension_with_schema() {
        let sql = r#"
CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA "extensions" VERSION '1.1';
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let ext = schema.extensions.get("uuid-ossp").expect("Extension not found");
        
        // Should be just "extensions", not "\"extensions\""
        assert_eq!(ext.schema, Some("extensions".to_string()));
        assert_eq!(ext.version, Some("1.1".to_string()));
    }

    #[test]
    fn test_parse_foreign_key_strips_quotes() {
        let sql = r#"
CREATE TABLE "users" ("id" UUID NOT NULL);
CREATE TABLE "posts" (
    "id" UUID NOT NULL,
    "user_id" UUID
);
ALTER TABLE "posts" ADD CONSTRAINT "posts_user_id_fkey" 
    FOREIGN KEY ("user_id") REFERENCES "users"("id") ON DELETE CASCADE;
"#;
        
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let posts = schema.tables.get("\"public\".\"posts\"").expect("Table not found");
        
        // FK should have bare/unquoted names (matching introspection behavior)
        let fk = &posts.foreign_keys[0];
        assert_eq!(fk.constraint_name, "posts_user_id_fkey"); // no quotes
        assert_eq!(fk.column_name, "user_id"); // no quotes  
        assert_eq!(fk.foreign_table, "users"); // no quotes
        assert_eq!(fk.foreign_column, "id"); // no quotes
        assert_eq!(fk.on_delete, "CASCADE");
    }
}
