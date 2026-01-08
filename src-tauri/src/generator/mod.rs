mod constraints;
mod objects;
mod roles;
mod tables;
mod types;

use crate::defaults;
use crate::diff::{EnumChangeType, SchemaDiff};
use crate::schema::{
    CompositeTypeInfo, DbSchema, DomainInfo, ExtensionInfo, RoleInfo, SequenceInfo, TableInfo, ViewInfo,
};

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
    // 0.5 SCHEMAS
    // ====================
    let mut schemas = std::collections::HashSet::new();

    for table in local_schema.tables.values() {
        schemas.insert(table.schema.clone());
    }
    for view in local_schema.views.values() {
        schemas.insert(view.schema.clone());
    }
    for enum_info in local_schema.enums.values() {
        schemas.insert(enum_info.schema.clone());
    }
    for seq in local_schema.sequences.values() {
        schemas.insert(seq.schema.clone());
    }
    for func in local_schema.functions.values() {
        schemas.insert(func.schema.clone());
    }
    for comp in local_schema.composite_types.values() {
        schemas.insert(comp.schema.clone());
    }
    for domain in local_schema.domains.values() {
        schemas.insert(domain.schema.clone());
    }
    // Extensions might specify a schema
    for ext in local_schema.extensions.values() {
        if let Some(s) = &ext.schema {
            schemas.insert(s.clone());
        }
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
                statements.push(format!("DROP MATERIALIZED VIEW IF EXISTS \"{}\";", name));
            } else {
                statements.push(format!("DROP VIEW IF EXISTS \"{}\";", name));
            }
        } else {
            // Default to regular view if not found
            statements.push(format!("DROP VIEW IF EXISTS \"{}\";", name));
        }
    }

    // Drop functions
    for name in &diff.functions_to_drop {
        statements.push(format!("DROP FUNCTION IF EXISTS \"{}\" CASCADE;", name));
    }

    // Drop sequences
    for name in &diff.sequences_to_drop {
        statements.push(format!("DROP SEQUENCE IF EXISTS {} CASCADE;", objects::ensure_quoted(name)));
    }

    // Drop tables
    for name in &diff.tables_to_drop {
        statements.push(format!("DROP TABLE IF EXISTS \"{}\" CASCADE;", name));
    }

    // Drop enums
    for enum_change in &diff.enum_changes {
        if enum_change.type_ == EnumChangeType::Drop {
            statements.push(format!("DROP TYPE IF EXISTS {} CASCADE;", objects::ensure_quoted(&enum_change.name)));
        }
    }

    // Drop composite types
    for name in &diff.composite_types_to_drop {
        statements.push(format!("DROP TYPE IF EXISTS {} CASCADE;", objects::ensure_quoted(name)));
    }

    // Drop domains
    for name in &diff.domains_to_drop {
        statements.push(format!("DROP DOMAIN IF EXISTS {} CASCADE;", objects::ensure_quoted(name)));
    }

    // Drop extensions (last, as others may depend on them)
    for name in &diff.extensions_to_drop {
        // Extensions keys might not be qualified? introspection says extensions usually global but name is unique.
        // parsing.rs uses just name.
        // So for extension, we keep quotes: "uuid-ossp".
        statements.push(format!("DROP EXTENSION IF EXISTS \"{}\" CASCADE;", name));
    }

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

    statements.join("\n")
}

pub fn escape_string(s: &str) -> String {
    s.replace('\'', "''")
}

#[cfg(test)]
mod tests;
