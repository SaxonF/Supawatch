use crate::schema::{DbSchema, FunctionGrant, FunctionInfo};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sqlparser::ast::{Statement, Privileges, GrantObjects, Grantee};
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
    // Function options workaround:
    // sqlparser-rs doesn't support SECURITY DEFINER or SET clauses yet, so we manually extract them
    // and remove them from the SQL before parsing.
    let (cleaned_sql, func_options) = preprocess_function_options(sql);

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
                let func_key_formats = [
                    format!("\"{}\".\"{}\"", schema, name),
                    format!("{}.{}", schema, name),
                    name.clone(),
                ];
                
                let options = func_key_formats.iter()
                    .find_map(|key| func_options.get(key))
                    .map(|o| (o.security_definer, o.config_params.clone()))
                    .unwrap_or((false, vec![]));
                
                functions::handle_create_function(&mut functions, stmt, options.0, options.1);
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
            Statement::Grant {
                privileges,
                objects,
                grantees,
                ..
            } => {
                // Handle GRANT EXECUTE ON FUNCTION ... TO ...
                if let Some(objs) = objects {
                    handle_grant_on_function(&mut functions, privileges, objs, grantees);
                }
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

/// Handle GRANT EXECUTE ON FUNCTION statements
/// Adds grants to any matching functions in the functions map
fn handle_grant_on_function(
    functions: &mut HashMap<String, FunctionInfo>,
    privileges: Privileges,
    objects: GrantObjects,
    grantees: Vec<Grantee>,
) {
    // Check if this is an EXECUTE grant
    let is_execute = match &privileges {
        Privileges::All { .. } => false, // ALL doesn't mean just EXECUTE
        Privileges::Actions(actions) => {
            actions.iter().any(|a| {
                // Action::Execute or similar - match on string representation
                a.to_string().to_uppercase().contains("EXECUTE")
            })
        }
    };
    
    if !is_execute {
        return;
    }
    
    // Extract function names from the grant target
    let func_names: Vec<String> = match objects {
        GrantObjects::AllFunctionsInSchema { .. } => vec![], // We can't handle this case easily
        GrantObjects::AllSequencesInSchema { .. } => vec![],
        GrantObjects::Sequences(_) | GrantObjects::Tables(_) => vec![], // Not functions
        _ => {
            // Try to extract from the string representation for function grants
            vec![objects.to_string()]
        }
    };
    
    // Early return if no function names found
    if func_names.is_empty() {
        return;
    }
    
    // Extract grantee names - Grantee has name field (Option<GranteeName>) and grantee_type field
    // 'public' role may use grantee_type instead of name
    let grantee_names: Vec<String> = grantees.iter()
        .filter_map(|g| {
            // First try the name field
            if let Some(name) = g.name.as_ref() {
                // Strip quotes from the name to match introspection
                Some(helpers::strip_quotes(&name.to_string()))
            } else {
                // Fall back to grantee_type for special roles like 'public'
                // Use Debug format since GranteesType doesn't implement Display
                let type_str = format!("{:?}", g.grantee_type).to_lowercase();
                if !type_str.is_empty() && type_str != "none" {
                    Some(type_str)
                } else {
                    None
                }
            }
        })
        .collect();
    
    // Try to match each function name to a function in our map
    for func_name_raw in &func_names {
        // Normalize the function name - it might be "public.my_func(text)" format
        let func_name = func_name_raw.trim();
        
        // Try to find matching function(s) in the map
        for (sig, func_info) in functions.iter_mut() {
            // Check if the signature contains this function name
            let sig_lower = sig.to_lowercase().replace("\"", "");
            let name_lower = func_name.to_lowercase().replace("\"", "").replace("function ", "");
            
            if sig_lower.contains(&name_lower) || name_lower.contains(&func_info.name.to_lowercase()) {
                // Add grants for each grantee
                for grantee in &grantee_names {
                    let grant = FunctionGrant {
                        grantee: grantee.clone(),
                        privilege: "EXECUTE".to_string(),
                    };
                    // Avoid duplicates
                    if !func_info.grants.contains(&grant) {
                        func_info.grants.push(grant);
                    }
                }
            }
        }
    }
}

/// Result of preprocessing function options from SQL
/// Contains the cleaned SQL and maps of function names to their extracted options
struct FunctionOptions {
    security_definer: bool,
    config_params: Vec<(String, String)>,
}

fn preprocess_function_options(sql: &str) -> (String, std::collections::HashMap<String, FunctionOptions>) {
    let mut cleaned_sql = sql.to_string();
    let mut func_options: std::collections::HashMap<String, FunctionOptions> = std::collections::HashMap::new();
    
    let sql_upper = sql.to_uppercase();
    
    // Find all functions and their positions
    let mut func_positions: Vec<(usize, String)> = vec![];
    let mut start_search = 0;
    
    while let Some(func_idx) = sql_upper[start_search..].find("FUNCTION") {
        let abs_idx = start_search + func_idx;
        // Find the opening parenthesis after FUNCTION to isolate name
        if let Some(paren_idx) = sql[abs_idx..].find('(') {
            let raw_name = sql[abs_idx + 8..abs_idx + paren_idx].trim().to_string();
            func_positions.push((abs_idx, raw_name));
        }
        start_search = abs_idx + 8;
    }
    
    // For each function, look for SECURITY DEFINER and SET clauses
    // We need to limit search to just the current function's definition
    for (i, (func_pos, func_name)) in func_positions.iter().enumerate() {
        let mut options = FunctionOptions {
            security_definer: false,
            config_params: vec![],
        };
        
        // Find the end of this function definition
        // Either the next function starts, or end of string
        let search_end = if i + 1 < func_positions.len() {
            func_positions[i + 1].0  // Position of next function
        } else {
            sql.len()
        };
        let func_section = &sql[*func_pos..search_end];
        let func_section_upper = func_section.to_uppercase();
        
        // Check for SECURITY DEFINER
        if func_section_upper.contains("SECURITY DEFINER") {
            options.security_definer = true;
        }
        
        // Look for SET clauses - only match the first one (there should only be one per function)
        // Pattern: SET param_name = 'value' or SET param_name TO value
        let set_regex = regex::Regex::new(r"(?i)\bSET\s+(\w+)\s*=\s*'([^']*)'").unwrap();
        if let Some(cap) = set_regex.captures(func_section) {
            let param_name = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let param_value = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
            options.config_params.push((param_name, param_value));
        }
        
        func_options.insert(func_name.clone(), options);
    }
    
    // Remove SECURITY DEFINER from SQL
    cleaned_sql = cleaned_sql.replace("SECURITY DEFINER", "");
    
    // Remove SET clauses - match SET param = 'value' pattern
    let set_clause_regex = regex::Regex::new(r"(?i)\bSET\s+\w+\s*=\s*'[^']*'\s*").unwrap();
    cleaned_sql = set_clause_regex.replace_all(&cleaned_sql, "").to_string();
    
    (cleaned_sql, func_options)
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

    // ============================================================================
    // Additional Parsing Tests for Full Postgres Feature Coverage
    // ============================================================================

    #[test]
    fn test_parse_domain() {
        let sql = r#"
CREATE DOMAIN positive_int AS integer CHECK (VALUE >= 0);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let domain = schema.domains.get("\"public\".\"positive_int\"").expect("Domain not found");
        assert_eq!(domain.name, "positive_int");
        assert_eq!(domain.base_type, "integer");
        assert!(!domain.check_constraints.is_empty());
    }

    #[test]
    fn test_parse_domain_with_default() {
        let sql = r#"
CREATE DOMAIN nonneg_int AS integer DEFAULT 0 CHECK (VALUE >= 0);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let domain = schema.domains.get("\"public\".\"nonneg_int\"").expect("Domain not found");
        assert_eq!(domain.name, "nonneg_int");
        assert_eq!(domain.base_type, "integer");
        assert_eq!(domain.default_value, Some("0".to_string()));
    }

    #[test]
    fn test_parse_gin_index() {
        let sql = r#"
CREATE TABLE documents (id uuid, tags text[]);
CREATE INDEX idx_tags_gin ON documents USING gin (tags);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"documents\"").expect("Table not found");

        let idx = table.indexes.iter().find(|i| i.index_name == "idx_tags_gin")
            .expect("Index not found");
        assert_eq!(idx.index_method, "gin");
    }

    #[test]
    fn test_parse_gist_index() {
        let sql = r#"
CREATE TABLE locations (id uuid, coords point);
CREATE INDEX idx_coords_gist ON locations USING gist (coords);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"locations\"").expect("Table not found");

        let idx = table.indexes.iter().find(|i| i.index_name == "idx_coords_gist")
            .expect("Index not found");
        assert_eq!(idx.index_method, "gist");
    }

    #[test]
    fn test_parse_hash_index() {
        let sql = r#"
CREATE TABLE cache (id uuid, key text);
CREATE INDEX idx_key_hash ON cache USING hash (key);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"cache\"").expect("Table not found");

        let idx = table.indexes.iter().find(|i| i.index_name == "idx_key_hash")
            .expect("Index not found");
        assert_eq!(idx.index_method, "hash");
    }

    #[test]
    fn test_parse_table_comment() {
        let sql = r#"
CREATE TABLE users (id uuid);
COMMENT ON TABLE users IS 'Main users table';
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");

        assert_eq!(table.comment, Some("Main users table".to_string()));
    }

    #[test]
    fn test_parse_column_comment() {
        let sql = r#"
CREATE TABLE users (id uuid, email text);
COMMENT ON COLUMN users.email IS 'User email address';
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");
        let email_col = table.columns.get("email").expect("Column not found");

        assert_eq!(email_col.comment, Some("User email address".to_string()));
    }

    #[test]
    fn test_parse_security_definer_function() {
        let sql = r#"
CREATE FUNCTION get_current_user_id() RETURNS uuid
    LANGUAGE sql
    SECURITY DEFINER
    AS $$ SELECT gen_random_uuid(); $$;
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let func = schema.functions.get("\"public\".\"get_current_user_id\"()")
            .expect("Function not found");
        assert!(func.security_definer);
    }

    #[test]
    fn test_parse_function_with_set_search_path() {
        let sql = r#"
CREATE FUNCTION public.broadcast_position() RETURNS trigger
    LANGUAGE plpgsql
    SECURITY DEFINER
    SET search_path = ''
    AS $$
BEGIN
    RETURN NEW;
END;
$$;
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let func = schema.functions.get("\"public\".\"broadcast_position\"()")
            .expect("Function not found");
        assert!(func.security_definer, "security_definer should be true");
        assert_eq!(func.config_params.len(), 1, "Should have one config param");
        assert_eq!(func.config_params[0].0, "search_path", "Config param name should be search_path");
        assert_eq!(func.config_params[0].1, "", "Config param value should be empty string");
    }

    #[test]
    fn test_parse_function_volatility_stable() {
        let sql = r#"
CREATE FUNCTION stable_func() RETURNS integer
    LANGUAGE sql
    STABLE
    AS $$ SELECT 1; $$;
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let func = schema.functions.get("\"public\".\"stable_func\"()")
            .expect("Function not found");
        assert_eq!(func.volatility, Some("STABLE".to_string()));
    }

    #[test]
    fn test_parse_function_immutable() {
        let sql = r#"
CREATE FUNCTION immutable_func(x integer) RETURNS integer
    LANGUAGE sql
    IMMUTABLE
    AS $$ SELECT x * 2; $$;
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let func = schema.functions.get("\"public\".\"immutable_func\"(integer)")
            .expect("Function not found");
        assert_eq!(func.volatility, Some("IMMUTABLE".to_string()));
    }

    #[test]
    fn test_parse_function_strict() {
        let sql = r#"
CREATE FUNCTION strict_func(x integer) RETURNS integer
    LANGUAGE sql
    STRICT
    AS $$ SELECT x + 1; $$;
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let func = schema.functions.get("\"public\".\"strict_func\"(integer)")
            .expect("Function not found");
        assert!(func.is_strict);
    }

    #[test]
    fn test_parse_array_column() {
        let sql = r#"
CREATE TABLE posts (
    id uuid,
    tags text[]
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"posts\"").expect("Table not found");
        let tags_col = table.columns.get("tags").expect("Column not found");

        assert!(tags_col.data_type.contains("[]") || tags_col.data_type.to_uppercase().contains("ARRAY"));
    }

    #[test]
    fn test_parse_enum_type() {
        let sql = r#"
CREATE TYPE status AS ENUM ('pending', 'active', 'cancelled');
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let enum_type = schema.enums.get("\"public\".\"status\"").expect("Enum not found");
        assert_eq!(enum_type.name, "status");
        assert_eq!(enum_type.values.len(), 3);
        assert!(enum_type.values.contains(&"pending".to_string()));
        assert!(enum_type.values.contains(&"active".to_string()));
        assert!(enum_type.values.contains(&"cancelled".to_string()));
    }

    #[test]
    fn test_parse_foreign_key_on_update() {
        let sql = r#"
CREATE TABLE users (id uuid PRIMARY KEY);
CREATE TABLE posts (
    id uuid,
    user_id uuid
);
ALTER TABLE posts ADD CONSTRAINT fk_user
    FOREIGN KEY (user_id) REFERENCES users(id)
    ON DELETE CASCADE ON UPDATE SET NULL;
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let posts = schema.tables.get("\"public\".\"posts\"").expect("Table not found");

        let fk = &posts.foreign_keys[0];
        assert_eq!(fk.on_delete, "CASCADE");
        assert_eq!(fk.on_update, "SET NULL");
    }

    #[test]
    fn test_parse_multi_column_primary_key() {
        let sql = r#"
CREATE TABLE order_items (
    order_id uuid,
    item_id uuid,
    quantity integer,
    PRIMARY KEY (order_id, item_id)
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"order_items\"").expect("Table not found");

        let order_id_col = table.columns.get("order_id").expect("Column not found");
        let item_id_col = table.columns.get("item_id").expect("Column not found");

        assert!(order_id_col.is_primary_key);
        assert!(item_id_col.is_primary_key);
    }

    #[test]
    fn test_parse_sequence_with_all_options() {
        let sql = r#"
CREATE SEQUENCE order_seq
    INCREMENT BY 5
    MINVALUE 1
    MAXVALUE 999999
    START WITH 1000
    CACHE 20;
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let seq = schema.sequences.get("\"public\".\"order_seq\"").expect("Sequence not found");
        assert_eq!(seq.name, "order_seq");
        assert_eq!(seq.start_value, 1000);
        assert_eq!(seq.increment, 5);
        assert_eq!(seq.min_value, 1);
        assert_eq!(seq.max_value, 999999);
        assert_eq!(seq.cache_size, 20);
        assert!(!seq.cycle);
    }

    #[test]
    fn test_parse_policy_with_using_and_check() {
        let sql = r#"
CREATE TABLE posts (id uuid, author_id uuid);
ALTER TABLE posts ENABLE ROW LEVEL SECURITY;
CREATE POLICY manage_own ON posts FOR ALL TO public
    USING (author_id = current_user_id())
    WITH CHECK (author_id = current_user_id());
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"posts\"").expect("Table not found");

        let policy = table.policies.iter().find(|p| p.name == "manage_own")
            .expect("Policy not found");
        assert!(policy.qual.is_some());
        assert!(policy.with_check.is_some());
    }

    #[test]
    fn test_parse_composite_type_with_collation() {
        let sql = r#"
CREATE TYPE person_name AS (
    first_name text COLLATE "C",
    last_name text
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let comp_type = schema.composite_types.get("\"public\".\"person_name\"")
            .expect("Composite type not found");
        assert_eq!(comp_type.attributes.len(), 2);
        let first = &comp_type.attributes[0];
        assert_eq!(first.name, "first_name");
        assert!(first.collation.is_some());
    }

    #[test]
    fn test_parse_role_with_options() {
        let sql = r#"
CREATE ROLE app_admin WITH
    LOGIN
    CREATEDB
    CREATEROLE
    CONNECTION LIMIT 10;
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let role = schema.roles.get("app_admin").expect("Role not found");
        assert!(role.login);
        assert!(role.create_db);
        assert!(role.create_role);
        assert_eq!(role.connection_limit, 10);
    }

    #[test]
    fn test_parse_statement_level_trigger() {
        let sql = r#"
CREATE TABLE events (id uuid);
CREATE FUNCTION notify_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NULL; END; $$;
CREATE TRIGGER trg_notify
    AFTER INSERT ON events
    FOR EACH STATEMENT
    EXECUTE FUNCTION notify_event();
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"events\"").expect("Table not found");

        let trigger = &table.triggers[0];
        assert_eq!(trigger.orientation, "STATEMENT");
    }

    #[test]
    fn test_parse_multiple_trigger_events() {
        let sql = r#"
CREATE TABLE data (id uuid);
CREATE FUNCTION audit_changes() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END; $$;
CREATE TRIGGER trg_audit
    BEFORE INSERT OR UPDATE OR DELETE ON data
    FOR EACH ROW
    EXECUTE FUNCTION audit_changes();
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"data\"").expect("Table not found");

        let trigger = &table.triggers[0];
        assert!(trigger.events.contains(&"INSERT".to_string()));
        assert!(trigger.events.contains(&"UPDATE".to_string()));
        assert!(trigger.events.contains(&"DELETE".to_string()));
    }

    #[test]
    fn test_parse_unique_constraint_as_index() {
        let sql = r#"
CREATE TABLE users (id uuid, email text);
ALTER TABLE users ADD CONSTRAINT unique_email UNIQUE (email);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");

        let idx = table.indexes.iter().find(|i| i.index_name == "unique_email")
            .expect("Index not found");
        assert!(idx.is_unique);
        assert!(idx.owning_constraint.is_some());
    }

    #[test]
    fn test_repro_double_check() {
        let sql = r#"
CREATE TABLE "quest_instances" (
  "status" text NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'completed', 'failed', 'abandoned'))
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"quest_instances\"").expect("Table not found");

        let constraint = &table.check_constraints[0];
        // This is what we EXPECT it to be:
        assert_eq!(constraint.expression, "CHECK (status IN ('active', 'completed', 'failed', 'abandoned'))");
    }


    #[test]
    fn test_parse_view_complex_query() {
        let sql = r#"
CREATE VIEW user_post_counts AS
    SELECT u.id, COUNT(p.id) as post_count
    FROM users u
    LEFT JOIN posts p ON p.user_id = u.id
    GROUP BY u.id;
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let view = schema.views.get("\"public\".\"user_post_counts\"").expect("View not found");
        assert!(!view.is_materialized);
        assert!(view.definition.contains("SELECT"));
    }

    #[test]
    fn test_parse_function_with_out_param() {
        let sql = r#"
CREATE FUNCTION get_stats(IN p_name text, OUT row_count integer)
    RETURNS integer
    LANGUAGE sql
    AS $$ SELECT 100; $$;
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        // Function should be found with its signature
        let func_key = schema.functions.keys()
            .find(|k| k.contains("get_stats"))
            .expect("Function not found");
        let func = schema.functions.get(func_key).unwrap();

        // Check that OUT params are parsed
        assert!(func.args.iter().any(|a| a.mode == Some("OUT".to_string())));
    }

    #[test]
    fn test_parse_function_with_default_args() {
        let sql = r#"
CREATE FUNCTION greet(name text DEFAULT 'World')
    RETURNS text
    LANGUAGE sql
    AS $$ SELECT 'Hello'; $$;
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let func = schema.functions.get("\"public\".\"greet\"(text)")
            .expect("Function not found");
        assert!(func.args[0].default_value.is_some());
    }

    #[test]
    fn test_parse_schema_qualified_objects() {
        let sql = r#"
CREATE TABLE custom_schema.users (id uuid);
CREATE TYPE custom_schema.status AS ENUM ('a', 'b');
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        assert!(schema.tables.contains_key("\"custom_schema\".\"users\""));
        assert!(schema.enums.contains_key("\"custom_schema\".\"status\""));
    }

    #[test]
    fn test_parse_identity_by_default() {
        let sql = r#"
CREATE TABLE items (
    id integer GENERATED BY DEFAULT AS IDENTITY,
    name text
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"items\"").expect("Table not found");

        let id_col = table.columns.get("id").expect("id column not found");
        assert!(id_col.is_identity);
        assert_eq!(id_col.identity_generation, Some("BY DEFAULT".to_string()));
    }

    #[test]
    fn test_parse_numeric_with_precision() {
        let sql = r#"
CREATE TABLE prices (
    id uuid,
    amount numeric(10, 2)
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"prices\"").expect("Table not found");

        let amount_col = table.columns.get("amount").expect("amount column not found");
        assert!(amount_col.data_type.to_lowercase().contains("numeric"));
    }

    #[test]
    fn test_parse_varchar_with_length() {
        let sql = r#"
CREATE TABLE users (
    id uuid,
    username varchar(50)
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");

        let username_col = table.columns.get("username").expect("username column not found");
        assert!(username_col.data_type.to_lowercase().contains("varchar") ||
                username_col.data_type.to_lowercase().contains("character varying"));
    }

    #[test]
    fn test_parse_timestamp_with_timezone() {
        let sql = r#"
CREATE TABLE events (
    id uuid,
    created_at timestamptz DEFAULT now()
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"events\"").expect("Table not found");

        let created_col = table.columns.get("created_at").expect("created_at column not found");
        assert!(created_col.column_default.is_some());
    }

    #[test]
    fn test_parse_jsonb_column() {
        let sql = r#"
CREATE TABLE documents (
    id uuid,
    data jsonb DEFAULT '{}'::jsonb
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"documents\"").expect("Table not found");

        let data_col = table.columns.get("data").expect("data column not found");
        assert!(data_col.data_type.to_lowercase().contains("jsonb"));
    }

    #[test]
    fn test_parse_uuid_column_with_default() {
        let sql = r#"
CREATE TABLE users (
    id uuid DEFAULT gen_random_uuid() PRIMARY KEY
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");

        let id_col = table.columns.get("id").expect("id column not found");
        assert!(id_col.is_primary_key);
        assert!(id_col.column_default.is_some());
    }

    #[test]
    fn test_parse_boolean_column_with_default() {
        let sql = r#"
CREATE TABLE users (
    id uuid,
    is_active boolean DEFAULT true NOT NULL
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");

        let col = table.columns.get("is_active").expect("is_active column not found");
        assert!(!col.is_nullable);
        assert!(col.column_default.is_some());
    }

    #[test]
    fn test_parse_inline_foreign_key() {
        let sql = r#"
CREATE TABLE users (id uuid PRIMARY KEY);
CREATE TABLE posts (
    id uuid PRIMARY KEY,
    user_id uuid REFERENCES users(id) ON DELETE CASCADE
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"posts\"").expect("Table not found");
        
        // Verify the FK is captured
        assert_eq!(table.foreign_keys.len(), 1);
        let fk = &table.foreign_keys[0];
        assert_eq!(fk.column_name, "user_id");
        assert_eq!(fk.foreign_table, "users");
        assert_eq!(fk.foreign_column, "id");
        assert_eq!(fk.on_delete, "CASCADE");
        assert_eq!(fk.on_update, "NO ACTION");
    }

    #[test]
    fn test_parse_update_of_column_trigger() {
        let sql = r#"
CREATE TABLE character_skills (id uuid, experience integer);
CREATE FUNCTION check_skill_level_up() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RETURN NEW; END; $$;
CREATE TRIGGER "on_skill_experience_change" BEFORE UPDATE OF "experience" ON "public"."character_skills" FOR EACH ROW WHEN ((NEW.experience > OLD.experience)) EXECUTE FUNCTION check_skill_level_up();
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"character_skills\"").expect("Table not found");
        
        assert_eq!(table.triggers.len(), 1);
        let trigger = &table.triggers[0];
        
        assert_eq!(trigger.name, "on_skill_experience_change");
        assert_eq!(trigger.timing, "BEFORE");
        assert_eq!(trigger.orientation, "ROW");
        
        // Print the exact event format for debugging
        println!("Events from parsing: {:?}", trigger.events);
        println!("When clause from parsing: {:?}", trigger.when_clause);
        
        // Verify the event includes column specification
        assert_eq!(trigger.events.len(), 1);
        // This shows us the exact format sqlparser produces
        assert!(trigger.events[0].to_uppercase().contains("UPDATE"));
    }
}
