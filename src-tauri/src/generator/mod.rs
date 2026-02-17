mod constraints;
mod objects;
mod roles;
mod tables;
pub mod typescript;
mod types;

use crate::defaults;
use crate::diff::{EnumChangeType, SchemaDiff};
use crate::schema::{
    CompositeTypeInfo, DbSchema, DomainInfo, ExtensionInfo, RoleInfo, SequenceInfo, TableInfo,
    ViewInfo,
};

/// Generate split SQL files from a `DbSchema`.
///
/// Takes a complete schema and produces a `Vec<(filename, sql_content)>` where each file
/// contains a specific category of SQL objects. Files are numbered so that alphabetical
/// sorting gives correct dependency order.
///
/// This is a standalone operation — it works on any `DbSchema`, whether parsed from a local
/// file or introspected from remote. It can be used during pull or independently to
/// "prettify" an existing schema.sql.
pub fn split_sql(schema: &DbSchema) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = Vec::new();

    // ---- 00_extensions.sql: CREATE SCHEMA + CREATE EXTENSION ----
    {
        let mut stmts: Vec<String> = Vec::new();

        // Collect schemas from all objects
        let mut schemas = std::collections::HashSet::new();
        for table in schema.tables.values() {
            schemas.insert(table.schema.clone());
        }
        for view in schema.views.values() {
            schemas.insert(view.schema.clone());
        }
        for enum_info in schema.enums.values() {
            schemas.insert(enum_info.schema.clone());
        }
        for seq in schema.sequences.values() {
            schemas.insert(seq.schema.clone());
        }
        for func in schema.functions.values() {
            schemas.insert(func.schema.clone());
        }
        for comp in schema.composite_types.values() {
            schemas.insert(comp.schema.clone());
        }
        for domain in schema.domains.values() {
            schemas.insert(domain.schema.clone());
        }
        for ext in schema.extensions.values() {
            if let Some(s) = &ext.schema {
                schemas.insert(s.clone());
            }
        }

        let mut sorted_schemas: Vec<String> = schemas.into_iter().collect();
        sorted_schemas.sort();
        for s in sorted_schemas {
            if !defaults::is_excluded_schema(&s) {
                stmts.push(format!("CREATE SCHEMA IF NOT EXISTS \"{}\";", s));
            }
        }

        // Extensions (sorted by name for deterministic output)
        let mut exts: Vec<&ExtensionInfo> = schema.extensions.values().collect();
        exts.sort_by(|a, b| a.name.cmp(&b.name));
        for ext in exts {
            stmts.push(roles::generate_create_extension(ext));
        }

        if !stmts.is_empty() {
            files.push(("00_extensions.sql".to_string(), stmts.join("\n")));
        }
    }

    // ---- 01_roles.sql ----
    {
        let mut stmts: Vec<String> = Vec::new();
        let mut role_list: Vec<&RoleInfo> = schema.roles.values().collect();
        role_list.sort_by(|a, b| a.name.cmp(&b.name));
        for role in role_list {
            stmts.push(roles::generate_create_role(role));
        }
        if !stmts.is_empty() {
            files.push(("01_roles.sql".to_string(), stmts.join("\n")));
        }
    }

    // ---- 02_types.sql: enums, composite types, domains ----
    {
        let mut stmts: Vec<String> = Vec::new();

        // Enums
        let mut enum_list: Vec<(&String, &crate::schema::EnumInfo)> =
            schema.enums.iter().collect();
        enum_list.sort_by(|a, b| a.0.cmp(b.0));
        for (name, enum_info) in enum_list {
            stmts.push(types::generate_create_enum(name, &enum_info.values));
        }

        // Composite types
        let mut comp_list: Vec<&CompositeTypeInfo> = schema.composite_types.values().collect();
        comp_list.sort_by(|a, b| a.name.cmp(&b.name));
        for comp in comp_list {
            stmts.push(types::generate_create_composite_type(comp));
        }

        // Domains
        let mut domain_list: Vec<&DomainInfo> = schema.domains.values().collect();
        domain_list.sort_by(|a, b| a.name.cmp(&b.name));
        for domain in domain_list {
            stmts.push(types::generate_create_domain(domain));
        }

        if !stmts.is_empty() {
            files.push(("02_types.sql".to_string(), stmts.join("\n")));
        }
    }

    // ---- 03_sequences.sql ----
    {
        let mut stmts: Vec<String> = Vec::new();
        let mut seq_list: Vec<&SequenceInfo> = schema.sequences.values().collect();
        seq_list.sort_by(|a, b| a.name.cmp(&b.name));
        for seq in seq_list {
            stmts.push(objects::generate_create_sequence(seq));
        }
        if !stmts.is_empty() {
            files.push(("03_sequences.sql".to_string(), stmts.join("\n")));
        }
    }

    // ---- 04_tables.sql: tables with their indexes, RLS, policies, triggers ----
    {
        let mut stmts: Vec<String> = Vec::new();
        let mut table_list: Vec<(&String, &TableInfo)> = schema.tables.iter().collect();
        table_list.sort_by(|a, b| a.0.cmp(b.0));

        for (name, table) in &table_list {
            // CREATE TABLE (includes indexes and RLS enable)
            stmts.push(tables::generate_create_table(table));

            // Policies
            let qualified_name = format!("\"{}\".\"{}\"\n", table.schema, table.table_name);
            let qualified_name = qualified_name.trim().to_string();
            for policy in &table.policies {
                stmts.push(constraints::generate_create_policy(&qualified_name, policy));
            }

            // Triggers
            for trigger in &table.triggers {
                stmts.push(constraints::generate_create_trigger(&qualified_name, trigger));
            }

            // Add a blank line between tables for readability
            stmts.push(String::new());
        }

        if !stmts.is_empty() {
            files.push(("04_tables.sql".to_string(), stmts.join("\n")));
        }
    }

    // ---- 05_views.sql ----
    {
        let mut stmts: Vec<String> = Vec::new();
        let mut view_list: Vec<&ViewInfo> = schema.views.values().collect();
        view_list.sort_by(|a, b| a.name.cmp(&b.name));
        for view in view_list {
            stmts.push(objects::generate_create_view(view));
        }
        if !stmts.is_empty() {
            files.push(("05_views.sql".to_string(), stmts.join("\n")));
        }
    }

    // ---- 06_functions.sql: functions + their grants ----
    {
        let mut stmts: Vec<String> = Vec::new();
        let mut func_list: Vec<&crate::schema::FunctionInfo> = schema.functions.values().collect();
        func_list.sort_by(|a, b| a.name.cmp(&b.name));
        for func in func_list {
            stmts.push(objects::generate_create_function(func));
            let grants = objects::generate_function_grants(func);
            stmts.extend(grants);
        }
        if !stmts.is_empty() {
            files.push(("06_functions.sql".to_string(), stmts.join("\n")));
        }
    }

    // ---- 07_foreign_keys.sql: all FK constraints deferred for dependency safety ----
    {
        let mut stmts: Vec<String> = Vec::new();
        let mut table_list: Vec<(&String, &TableInfo)> = schema.tables.iter().collect();
        table_list.sort_by(|a, b| a.0.cmp(b.0));

        for (_, table) in &table_list {
            let qualified_name = format!("\"{}\".\"{}\"\n", table.schema, table.table_name);
            let qualified_name = qualified_name.trim().to_string();
            for fk in &table.foreign_keys {
                stmts.push(constraints::generate_add_foreign_key(&qualified_name, fk));
            }
        }

        if !stmts.is_empty() {
            files.push(("07_foreign_keys.sql".to_string(), stmts.join("\n")));
        }
    }

    // ---- 08_comments.sql ----
    {
        let mut stmts: Vec<String> = Vec::new();

        // Table and column comments
        let mut table_list: Vec<(&String, &TableInfo)> = schema.tables.iter().collect();
        table_list.sort_by(|a, b| a.0.cmp(b.0));
        for (name, table) in &table_list {
            if let Some(comment) = &table.comment {
                stmts.push(format!(
                    "COMMENT ON TABLE \"{}\" IS '{}';",
                    name,
                    escape_string(comment)
                ));
            }
            let mut columns: Vec<_> = table.columns.values().collect();
            columns.sort_by(|a, b| a.column_name.cmp(&b.column_name));
            for col in columns {
                if let Some(comment) = &col.comment {
                    stmts.push(format!(
                        "COMMENT ON COLUMN \"{}\".\"{}\" IS '{}';",
                        name,
                        col.column_name,
                        escape_string(comment)
                    ));
                }
            }
        }

        // View comments
        let mut view_list: Vec<&ViewInfo> = schema.views.values().collect();
        view_list.sort_by(|a, b| a.name.cmp(&b.name));
        for view in view_list {
            if let Some(comment) = &view.comment {
                let view_type = if view.is_materialized {
                    "MATERIALIZED VIEW"
                } else {
                    "VIEW"
                };
                stmts.push(format!(
                    "COMMENT ON {} \"{}\" IS '{}';",
                    view_type,
                    view.name,
                    escape_string(comment)
                ));
            }
        }

        if !stmts.is_empty() {
            files.push(("08_comments.sql".to_string(), stmts.join("\n")));
        }
    }

    files
}

pub fn generate_sql(diff: &SchemaDiff, local_schema: &DbSchema) -> String {
    let mut statements: Vec<String> = vec![];

    // Order matters! Follow dependency order:
    // 1. Extensions (needed by everything)
    // 2. Drop dependent objects first (reverse dependency order)
    // 3. Types (domains, composite types, enums)
    // 4. Sequences (before tables that use them)
    // 5. Tables
    // 6. Views (depend on tables)
    // 7. Functions
    // 8. Triggers, Policies, etc.
    // 9. Foreign keys (deferred to end)

    // ====================
    // 0. ROLES (Global objects)
    // ====================

    // Drop roles
    for name in &diff.roles_to_drop {
        statements.push(format!("DROP ROLE IF EXISTS \"{}\";", name));
    }

    // Create roles
    for role in &diff.roles_to_create {
        statements.push(roles::generate_create_role(role));
    }

    // Update roles
    for role in &diff.roles_to_update {
        statements.push(roles::generate_alter_role(role));
    }

    // ====================
    // 0.5 SCHEMAS — only create schemas needed by NEW objects
    // ====================
    let mut schemas = std::collections::HashSet::new();

    // Collect schemas from objects that are being CREATED (not all local objects)
    for name in &diff.tables_to_create {
        if let Some(table) = local_schema.tables.get(name) {
            schemas.insert(table.schema.clone());
        }
    }
    for view in &diff.views_to_create {
        schemas.insert(view.schema.clone());
    }
    for enum_change in &diff.enum_changes {
        if enum_change.type_ == EnumChangeType::Create {
            // Enum name may be schema-qualified; extract schema
            if let Some(dot_pos) = enum_change.name.rfind('.') {
                let schema_part = &enum_change.name[..dot_pos];
                let cleaned = schema_part.trim_matches('"');
                schemas.insert(cleaned.to_string());
            }
        }
    }
    for seq in &diff.sequences_to_create {
        schemas.insert(seq.schema.clone());
    }
    for func in &diff.functions_to_create {
        schemas.insert(func.schema.clone());
    }
    for comp in &diff.composite_types_to_create {
        schemas.insert(comp.schema.clone());
    }
    for domain in &diff.domains_to_create {
        schemas.insert(domain.schema.clone());
    }

    let mut sorted_schemas: Vec<String> = schemas.into_iter().collect();
    sorted_schemas.sort();

    for schema in sorted_schemas {
        if !defaults::is_excluded_schema(&schema) {
            statements.push(format!("CREATE SCHEMA IF NOT EXISTS \"{}\";", schema));
        }
    }

    // ====================
    // 1. EXTENSIONS
    // ====================
    for ext in &diff.extensions_to_create {
        statements.push(roles::generate_create_extension(ext));
    }

    // ====================
    // 2. DROP operations (reverse dependency order)
    // ====================

    // Drop views first (depend on tables)
    for name in &diff.views_to_drop {
        // Check if it was a materialized view in the local schema
        if let Some(view) = local_schema.views.get(name) {
            if view.is_materialized {
                statements.push(format!("DROP MATERIALIZED VIEW IF EXISTS {};", name));
            } else {
                statements.push(format!("DROP VIEW IF EXISTS {};", name));
            }
        } else {
            // Default to regular view if not found
            statements.push(format!("DROP VIEW IF EXISTS {};", name));
        }
    }

    // Drop functions
    for name in &diff.functions_to_drop {
        statements.push(format!("DROP FUNCTION IF EXISTS {} CASCADE;", name));
    }

    // Drop sequences
    for name in &diff.sequences_to_drop {
        statements.push(format!("DROP SEQUENCE IF EXISTS {} CASCADE;", objects::ensure_quoted(name)));
    }

    // Drop tables
    for name in &diff.tables_to_drop {
        statements.push(format!("DROP TABLE IF EXISTS {} CASCADE;", name));
    }

    // Drop enums - MOVED TO END
    // Drop composite types - MOVED TO END
    // Drop domains - MOVED TO END
    // Drop extensions - MOVED TO END

    // ====================
    // 3. TYPES (domains, composite types, enums)
    // ====================

    // Create enums
    for enum_change in &diff.enum_changes {
        if enum_change.type_ == EnumChangeType::Create {
            if let Some(values) = &enum_change.values_to_add {
                statements.push(types::generate_create_enum(&enum_change.name, values));
            }
        }
    }

    // Create composite types
    for comp in &diff.composite_types_to_create {
        statements.push(types::generate_create_composite_type(comp));
    }

    // Create domains
    for domain in &diff.domains_to_create {
        statements.push(types::generate_create_domain(domain));
    }

    // Add enum values
    for enum_change in &diff.enum_changes {
        if enum_change.type_ == EnumChangeType::AddValue {
            if let Some(new_values) = &enum_change.values_to_add {
                for value in new_values {
                    statements.push(format!(
                        "ALTER TYPE \"{}\" ADD VALUE IF NOT EXISTS '{}';",
                        enum_change.name, value
                    ));
                }
            }
        }
    }

    // ====================
    // 4. SEQUENCES
    // ====================
    for seq in &diff.sequences_to_create {
        statements.push(objects::generate_create_sequence(seq));
    }

    for seq in &diff.sequences_to_update {
        statements.push(objects::generate_alter_sequence(seq));
    }

    // ====================
    // 5. TABLES
    // ====================

    // Create functions first (triggers may depend on them)
    for func in &diff.functions_to_create {
        statements.push(objects::generate_create_function(func));
    }

    for func in &diff.functions_to_update {
        statements.push(objects::generate_create_function(func));
    }

    // Create new tables
    for name in &diff.tables_to_create {
        if let Some(table) = local_schema.tables.get(name) {
            statements.push(tables::generate_create_table(table));
        }
    }

    // Alter existing tables
    for (table_name, table_diff) in &diff.table_changes {
        if let Some(table) = local_schema.tables.get(table_name) {
            let alter_stmts = tables::generate_alter_table(table_name, table_diff, table);
            statements.extend(alter_stmts);
        }
    }

    // ====================
    // 6. VIEWS
    // ====================

    // Update existing views (drop + create for materialized, replace for regular)
    for view in &diff.views_to_update {
        if view.is_materialized {
            statements.push(format!(
                "DROP MATERIALIZED VIEW IF EXISTS \"{}\";",
                view.name
            ));
        }
        statements.push(objects::generate_create_view(view));
    }

    // Create new views
    for view in &diff.views_to_create {
        statements.push(objects::generate_create_view(view));
    }

    // ====================
    // 7. POST-TABLE OPERATIONS
    // ====================

    // Add triggers for new tables
    for name in &diff.tables_to_create {
        if let Some(table) = local_schema.tables.get(name) {
            for trigger in &table.triggers {
                statements.push(constraints::generate_create_trigger(name, trigger));
            }
            for policy in &table.policies {
                statements.push(constraints::generate_create_policy(name, policy));
            }
        }
    }

    // ====================
    // 8. FOREIGN KEYS (deferred to end for dependency resolution)
    // ====================
    for name in &diff.tables_to_create {
        if let Some(table) = local_schema.tables.get(name) {
            for fk in &table.foreign_keys {
                statements.push(constraints::generate_add_foreign_key(name, fk));
            }
        }
    }

    // Foreign keys for modified tables
    for (table_name, table_diff) in &diff.table_changes {
        for fk in &table_diff.foreign_keys_to_create {
            statements.push(constraints::generate_add_foreign_key(table_name, fk));
        }
    }

    // ====================
    // 9. COMMENTS
    // ====================
    for name in &diff.tables_to_create {
        if let Some(table) = local_schema.tables.get(name) {
            if let Some(comment) = &table.comment {
                statements.push(format!(
                    "COMMENT ON TABLE \"{}\" IS '{}';",
                    name,
                    escape_string(comment)
                ));
            }
            // Column comments
            for col in table.columns.values() {
                if let Some(comment) = &col.comment {
                    statements.push(format!(
                        "COMMENT ON COLUMN \"{}\".\"{}\" IS '{}';",
                        name,
                        col.column_name,
                        escape_string(comment)
                    ));
                }
            }
        }
    }

    // Comment changes for existing tables
    for (table_name, table_diff) in &diff.table_changes {
        if let Some(new_comment) = &table_diff.comment_change {
            if let Some(comment) = new_comment {
                statements.push(format!(
                    "COMMENT ON TABLE \"{}\" IS '{}';",
                    table_name,
                    escape_string(comment)
                ));
            } else {
                statements.push(format!("COMMENT ON TABLE \"{}\" IS NULL;", table_name));
            }
        }

        // Column comment changes
        for mod_col in &table_diff.columns_to_modify {
            if let Some((_, new_comment)) = &mod_col.changes.comment_change {
                if let Some(comment) = new_comment {
                    statements.push(format!(
                        "COMMENT ON COLUMN \"{}\".\"{}\" IS '{}';",
                        table_name,
                        mod_col.column_name,
                        escape_string(comment)
                    ));
                } else {
                    statements.push(format!(
                        "COMMENT ON COLUMN \"{}\".\"{}\" IS NULL;",
                        table_name, mod_col.column_name
                    ));
                }
            }
        }
    }

    // View comments
    for view in &diff.views_to_create {
        if let Some(comment) = &view.comment {
            let view_type = if view.is_materialized {
                "MATERIALIZED VIEW"
            } else {
                "VIEW"
            };
            statements.push(format!(
                "COMMENT ON {} \"{}\" IS '{}';",
                view_type,
                view.name,
                escape_string(comment)
            ));
        }
    }

    // ====================
    // 10. DROP TYPES & EXTENSIONS (deferred to end to allow migration away from them first)
    // ====================

    // Drop domains
    for name in &diff.domains_to_drop {
        statements.push(format!("DROP DOMAIN IF EXISTS {} CASCADE;", objects::ensure_quoted(name)));
    }

    // Drop composite types
    for name in &diff.composite_types_to_drop {
        statements.push(format!("DROP TYPE IF EXISTS {} CASCADE;", objects::ensure_quoted(name)));
    }

    // Drop enums
    for enum_change in &diff.enum_changes {
        if enum_change.type_ == EnumChangeType::Drop {
            statements.push(format!("DROP TYPE IF EXISTS {} CASCADE;", objects::ensure_quoted(&enum_change.name)));
        }
    }

    // Drop extensions (last, as others may depend on them)
    for name in &diff.extensions_to_drop {
        statements.push(format!("DROP EXTENSION IF EXISTS \"{}\" CASCADE;", name));
    }

    statements.join("\n")
}

pub fn escape_string(s: &str) -> String {
    s.replace('\'', "''")
}

#[cfg(test)]
mod tests;

