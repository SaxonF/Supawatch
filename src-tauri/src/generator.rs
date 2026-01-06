use crate::diff::{EnumChange, EnumChangeType, SchemaDiff, TableDiff};
use crate::schema::{
    CheckConstraintInfo, CompositeTypeInfo, DbSchema, DomainInfo, ExtensionInfo, ForeignKeyInfo,
    FunctionInfo, IndexInfo, PolicyInfo, RoleInfo, SequenceInfo, TableInfo, TriggerInfo, ViewInfo,
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
        statements.push(generate_create_role(role));
    }

    // Update roles
    for role in &diff.roles_to_update {
        statements.push(generate_alter_role(role));
    }

    // ====================
    // 1. EXTENSIONS
    // ====================
    for ext in &diff.extensions_to_create {
        statements.push(generate_create_extension(ext));
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
            statements.push(format!("DROP VIEW IF EXISTS \"{}\";", name));
        }
    }

    // Drop functions
    for name in &diff.functions_to_drop {
        statements.push(format!("DROP FUNCTION IF EXISTS {} CASCADE;", name));
    }

    // Drop sequences
    for name in &diff.sequences_to_drop {
        statements.push(format!("DROP SEQUENCE IF EXISTS \"{}\";", name));
    }

    // Drop tables
    for name in &diff.tables_to_drop {
        statements.push(format!("DROP TABLE IF EXISTS \"{}\" CASCADE;", name));
    }

    // Drop enums
    for enum_change in &diff.enum_changes {
        if enum_change.type_ == EnumChangeType::Drop {
            statements.push(format!(
                "DROP TYPE IF EXISTS \"{}\" CASCADE;",
                enum_change.name
            ));
        }
    }

    // Drop composite types
    for name in &diff.composite_types_to_drop {
        statements.push(format!("DROP TYPE IF EXISTS \"{}\" CASCADE;", name));
    }

    // Drop domains
    for name in &diff.domains_to_drop {
        statements.push(format!("DROP DOMAIN IF EXISTS \"{}\" CASCADE;", name));
    }

    // Drop extensions (last, as others may depend on them)
    for name in &diff.extensions_to_drop {
        statements.push(format!("DROP EXTENSION IF EXISTS \"{}\" CASCADE;", name));
    }

    // ====================
    // 3. TYPES (domains, composite types, enums)
    // ====================

    // Create domains
    for domain in &diff.domains_to_create {
        statements.push(generate_create_domain(domain));
    }

    // Create composite types
    for comp_type in &diff.composite_types_to_create {
        statements.push(generate_create_composite_type(comp_type));
    }

    // Create enums
    for enum_change in &diff.enum_changes {
        if enum_change.type_ == EnumChangeType::Create {
            if let Some(values) = &enum_change.values {
                statements.push(generate_create_enum(&enum_change.name, values));
            }
        }
    }

    // Add enum values
    for enum_change in &diff.enum_changes {
        if enum_change.type_ == EnumChangeType::AddValue {
            if let Some(values_to_add) = &enum_change.values_to_add {
                for value in values_to_add {
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
        statements.push(generate_create_sequence(seq));
    }

    for seq in &diff.sequences_to_update {
        statements.push(generate_alter_sequence(seq));
    }

    // ====================
    // 5. TABLES
    // ====================

    // Create functions first (triggers may depend on them)
    for func in &diff.functions_to_create {
        statements.push(generate_create_function(func));
    }

    for func in &diff.functions_to_update {
        statements.push(generate_create_function(func));
    }

    // Create new tables
    for name in &diff.tables_to_create {
        if let Some(table) = local_schema.tables.get(name) {
            statements.push(generate_create_table(table));
        }
    }

    // Alter existing tables
    for (table_name, table_diff) in &diff.table_changes {
        if let Some(table) = local_schema.tables.get(table_name) {
            let alter_stmts = generate_alter_table(table_name, table_diff, table);
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
        statements.push(generate_create_view(view));
    }

    // Create new views
    for view in &diff.views_to_create {
        statements.push(generate_create_view(view));
    }

    // ====================
    // 7. POST-TABLE OPERATIONS
    // ====================

    // Add triggers for new tables
    for name in &diff.tables_to_create {
        if let Some(table) = local_schema.tables.get(name) {
            for trigger in &table.triggers {
                statements.push(generate_create_trigger(name, trigger));
            }
            for policy in &table.policies {
                statements.push(generate_create_policy(name, policy));
            }
        }
    }

    // ====================
    // 8. FOREIGN KEYS (deferred to end for dependency resolution)
    // ====================
    for name in &diff.tables_to_create {
        if let Some(table) = local_schema.tables.get(name) {
            for fk in &table.foreign_keys {
                statements.push(generate_add_foreign_key(name, fk));
            }
        }
    }

    // Foreign keys for modified tables
    for (table_name, table_diff) in &diff.table_changes {
        for fk in &table_diff.foreign_keys_to_create {
            statements.push(generate_add_foreign_key(table_name, fk));
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

fn generate_create_extension(ext: &ExtensionInfo) -> String {
    let mut sql = format!("CREATE EXTENSION IF NOT EXISTS \"{}\"", ext.name);
    if let Some(schema) = &ext.schema {
        sql.push_str(&format!(" WITH SCHEMA \"{}\"", schema));
    }
    if let Some(version) = &ext.version {
        sql.push_str(&format!(" VERSION '{}'", version));
    }
    sql.push(';');
    sql
}

fn generate_create_domain(domain: &DomainInfo) -> String {
    let mut sql = format!("CREATE DOMAIN \"{}\" AS {}", domain.name, domain.base_type);

    if let Some(collation) = &domain.collation {
        sql.push_str(&format!(" COLLATE \"{}\"", collation));
    }

    if let Some(default) = &domain.default_value {
        sql.push_str(&format!(" DEFAULT {}", default));
    }

    if domain.is_not_null {
        sql.push_str(" NOT NULL");
    }

    for check in &domain.check_constraints {
        if let Some(name) = &check.name {
            sql.push_str(&format!(" CONSTRAINT \"{}\"", name));
        }
        sql.push_str(&format!(" {}", check.expression));
    }

    sql.push(';');
    sql
}

fn generate_create_composite_type(comp_type: &CompositeTypeInfo) -> String {
    let attrs: Vec<String> = comp_type
        .attributes
        .iter()
        .map(|a| {
            let mut attr_sql = format!("\"{}\" {}", a.name, a.data_type);
            if let Some(collation) = &a.collation {
                attr_sql.push_str(&format!(" COLLATE \"{}\"", collation));
            }
            attr_sql
        })
        .collect();

    format!(
        "CREATE TYPE \"{}\" AS (\n  {}\n);",
        comp_type.name,
        attrs.join(",\n  ")
    )
}

fn generate_create_enum(name: &str, values: &[String]) -> String {
    let quoted_values: Vec<String> = values.iter().map(|v| format!("'{}'", v)).collect();
    format!(
        "CREATE TYPE \"{}\" AS ENUM ({});",
        name,
        quoted_values.join(", ")
    )
}

fn generate_create_sequence(seq: &SequenceInfo) -> String {
    let mut sql = format!("CREATE SEQUENCE \"{}\"", seq.name);

    if seq.data_type != "bigint" {
        sql.push_str(&format!(" AS {}", seq.data_type));
    }

    sql.push_str(&format!(" START WITH {}", seq.start_value));
    sql.push_str(&format!(" INCREMENT BY {}", seq.increment));
    sql.push_str(&format!(" MINVALUE {}", seq.min_value));
    sql.push_str(&format!(" MAXVALUE {}", seq.max_value));
    sql.push_str(&format!(" CACHE {}", seq.cache_size));

    if seq.cycle {
        sql.push_str(" CYCLE");
    } else {
        sql.push_str(" NO CYCLE");
    }

    if let Some(owned_by) = &seq.owned_by {
        sql.push_str(&format!(" OWNED BY {}", owned_by));
    }

    sql.push(';');
    sql
}

fn generate_alter_sequence(seq: &SequenceInfo) -> String {
    let mut parts = vec![];

    parts.push(format!("INCREMENT BY {}", seq.increment));
    parts.push(format!("MINVALUE {}", seq.min_value));
    parts.push(format!("MAXVALUE {}", seq.max_value));
    parts.push(format!("CACHE {}", seq.cache_size));

    if seq.cycle {
        parts.push("CYCLE".to_string());
    } else {
        parts.push("NO CYCLE".to_string());
    }

    format!("ALTER SEQUENCE \"{}\" {};", seq.name, parts.join(" "))
}

fn generate_create_view(view: &ViewInfo) -> String {
    let mut sql = String::new();

    if view.is_materialized {
        sql.push_str(&format!("CREATE MATERIALIZED VIEW \"{}\"", view.name));
    } else {
        sql.push_str(&format!("CREATE OR REPLACE VIEW \"{}\"", view.name));
    }

    if !view.with_options.is_empty() {
        sql.push_str(&format!(" WITH ({})", view.with_options.join(", ")));
    }

    sql.push_str(&format!(" AS {}", view.definition));

    if let Some(check) = &view.check_option {
        sql.push_str(&format!(" WITH {} CHECK OPTION", check));
    }

    if !sql.ends_with(';') {
        sql.push(';');
    }

    sql
}

fn generate_create_table(table: &TableInfo) -> String {
    let mut col_defs: Vec<String> = Vec::new();

    // Sort columns for deterministic output
    let mut columns: Vec<_> = table.columns.values().collect();
    columns.sort_by(|a, b| a.column_name.cmp(&b.column_name));

    // Primary key columns
    let pk_columns: Vec<String> = columns
        .iter()
        .filter(|c| c.is_primary_key)
        .map(|c| format!("\"{}\"", c.column_name))
        .collect();

    println!(
        "[DEBUG] Table {}: PK columns found: {:?}",
        table.table_name, pk_columns
    );

    for col in &columns {
        let mut col_sql = format!("\"{}\" {}", col.column_name, col.data_type);

        if let Some(collation) = &col.collation {
            col_sql.push_str(&format!(" COLLATE \"{}\"", collation));
        }

        if !col.is_nullable && !col.is_primary_key {
            col_sql.push_str(" NOT NULL");
        }

        if let Some(def) = &col.column_default {
            col_sql.push_str(&format!(" DEFAULT {}", def));
        }

        if let Some(identity) = &col.identity_generation {
            col_sql.push_str(&format!(" GENERATED {} AS IDENTITY", identity));
        }

        col_defs.push(col_sql);
    }

    // Primary key constraint
    if !pk_columns.is_empty() {
        col_defs.push(format!("PRIMARY KEY ({})", pk_columns.join(", ")));
    }

    // Check constraints
    for check in &table.check_constraints {
        col_defs.push(format!(
            "CONSTRAINT \"{}\" {}",
            check.name, check.expression
        ));
    }

    let mut sql = format!(
        "CREATE TABLE \"{}\" (\n  {}\n);",
        table.table_name,
        col_defs.join(",\n  ")
    );

    // Indexes (non-primary)
    println!(
        "[DEBUG] Table {} has {} indexes to create",
        table.table_name,
        table.indexes.len()
    );
    for idx in &table.indexes {
        println!(
            "[DEBUG] Generating index {}: is_primary={}, is_unique={}",
            idx.index_name, idx.is_primary, idx.is_unique
        );
        if !idx.is_primary {
            sql.push('\n');
            sql.push_str(&generate_create_index(&table.table_name, idx));
        }
    }

    // RLS
    if table.rls_enabled {
        sql.push_str(&format!(
            "\nALTER TABLE \"{}\" ENABLE ROW LEVEL SECURITY;",
            table.table_name
        ));
    }

    sql
}

fn generate_create_index(table_name: &str, idx: &IndexInfo) -> String {
    let mut sql = if idx.is_unique {
        format!("CREATE UNIQUE INDEX \"{}\"", idx.index_name)
    } else {
        format!("CREATE INDEX \"{}\"", idx.index_name)
    };

    sql.push_str(&format!(" ON \"{}\"", table_name));

    // Index method (if not btree)
    if idx.index_method.to_lowercase() != "btree" {
        sql.push_str(&format!(" USING {}", idx.index_method));
    }

    // Columns or expressions
    if !idx.expressions.is_empty() {
        sql.push_str(&format!(" ({})", idx.expressions.join(", ")));
    } else {
        let cols: Vec<String> = idx.columns.iter().map(|c| format!("\"{}\"", c)).collect();
        sql.push_str(&format!(" ({})", cols.join(", ")));
    }

    // WHERE clause for partial indexes
    if let Some(where_clause) = &idx.where_clause {
        sql.push_str(&format!(" WHERE {}", where_clause));
    }

    sql.push(';');
    sql
}

fn generate_create_trigger(table_name: &str, trigger: &TriggerInfo) -> String {
    let events = trigger.events.join(" OR ");

    let mut sql = format!(
        "CREATE TRIGGER \"{}\" {} {} ON \"{}\" FOR EACH {} ",
        trigger.name, trigger.timing, events, table_name, trigger.orientation
    );

    // WHEN clause
    if let Some(when) = &trigger.when_clause {
        sql.push_str(&format!("WHEN ({}) ", when));
    }

    sql.push_str(&format!("EXECUTE FUNCTION {}();", trigger.function_name));

    sql
}

fn generate_create_policy(table_name: &str, policy: &PolicyInfo) -> String {
    let mut sql = format!(
        "CREATE POLICY \"{}\" ON \"{}\" FOR {} TO {}",
        policy.name,
        table_name,
        policy.cmd,
        policy.roles.join(", ")
    );

    if let Some(q) = &policy.qual {
        sql.push_str(&format!(" USING ({})", q));
    }

    if let Some(wc) = &policy.with_check {
        sql.push_str(&format!(" WITH CHECK ({})", wc));
    }

    sql.push(';');
    sql
}

fn generate_create_function(func: &FunctionInfo) -> String {
    let args: Vec<String> = func
        .args
        .iter()
        .map(|a| {
            let mut arg_sql = String::new();
            if let Some(mode) = &a.mode {
                arg_sql.push_str(mode);
                arg_sql.push(' ');
            }
            if !a.name.is_empty() {
                arg_sql.push_str(&format!("{} ", a.name));
            }
            arg_sql.push_str(&a.type_);
            if let Some(default) = &a.default_value {
                arg_sql.push_str(&format!(" DEFAULT {}", default));
            }
            arg_sql
        })
        .collect();

    let mut sql = format!(
        "CREATE OR REPLACE FUNCTION {}({}) RETURNS {} LANGUAGE {} ",
        func.name,
        args.join(", "),
        func.return_type,
        func.language
    );

    // Volatility
    if let Some(vol) = &func.volatility {
        sql.push_str(vol);
        sql.push(' ');
    }

    // Strictness
    if func.is_strict {
        sql.push_str("STRICT ");
    }

    // Security
    if func.security_definer {
        sql.push_str("SECURITY DEFINER ");
    }

    sql.push_str(&format!("AS $${}$$;", func.definition));

    sql
}

fn generate_add_foreign_key(table_name: &str, fk: &ForeignKeyInfo) -> String {
    let mut sql = format!(
        "ALTER TABLE \"{}\" ADD CONSTRAINT \"{}\" FOREIGN KEY (\"{}\") REFERENCES \"{}\"(\"{}\")",
        table_name, fk.constraint_name, fk.column_name, fk.foreign_table, fk.foreign_column
    );

    if fk.on_delete != "NO ACTION" {
        sql.push_str(&format!(" ON DELETE {}", fk.on_delete));
    }

    if fk.on_update != "NO ACTION" {
        sql.push_str(&format!(" ON UPDATE {}", fk.on_update));
    }

    sql.push(';');
    sql
}

fn generate_alter_table(
    table_name: &str,
    diff: &TableDiff,
    local_table: &TableInfo,
) -> Vec<String> {
    let mut statements = vec![];

    // Drop foreign keys first (before dropping columns they reference)
    for fk in &diff.foreign_keys_to_drop {
        statements.push(format!(
            "ALTER TABLE \"{}\" DROP CONSTRAINT IF EXISTS \"{}\";",
            table_name, fk.constraint_name
        ));
    }

    // Drop check constraints
    for check in &diff.check_constraints_to_drop {
        statements.push(format!(
            "ALTER TABLE \"{}\" DROP CONSTRAINT IF EXISTS \"{}\";",
            table_name, check.name
        ));
    }

    // Drop policies
    for p in &diff.policies_to_drop {
        statements.push(format!(
            "DROP POLICY IF EXISTS \"{}\" ON \"{}\";",
            p.name, table_name
        ));
    }

    // Drop triggers
    for t in &diff.triggers_to_drop {
        statements.push(format!(
            "DROP TRIGGER IF EXISTS \"{}\" ON \"{}\";",
            t.name, table_name
        ));
    }

    // Drop indexes
    for i in &diff.indexes_to_drop {
        if let Some(constraint) = &i.owning_constraint {
            statements.push(format!(
                "ALTER TABLE \"{}\" DROP CONSTRAINT IF EXISTS \"{}\";",
                table_name, constraint
            ));
        } else {
            statements.push(format!("DROP INDEX IF EXISTS \"{}\";", i.index_name));
        }
    }

    // Drop columns
    for col in &diff.columns_to_drop {
        statements.push(format!(
            "ALTER TABLE \"{}\" DROP COLUMN IF EXISTS \"{}\";",
            table_name, col
        ));
    }

    // Add columns
    for col_name in &diff.columns_to_add {
        if let Some(col) = local_table.columns.get(col_name) {
            let mut add_sql = format!(
                "ALTER TABLE \"{}\" ADD COLUMN \"{}\" {}",
                table_name, col.column_name, col.data_type
            );

            if !col.is_nullable {
                add_sql.push_str(" NOT NULL");
            }

            if let Some(def) = &col.column_default {
                add_sql.push_str(&format!(" DEFAULT {}", def));
            }

            add_sql.push(';');
            statements.push(add_sql);
        }
    }

    // Modify columns
    for mod_col in &diff.columns_to_modify {
        let col_name = &mod_col.column_name;

        // Type Change
        if let Some((_, new_type)) = &mod_col.changes.type_change {
            let mut alter_sql = format!(
                "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" TYPE {} USING \"{}\"::{}",
                table_name, col_name, new_type, col_name, new_type
            );
            // If collation changed, apply it with TYPE change
            if let Some((_, Some(new_collation))) = &mod_col.changes.collation_change {
                alter_sql.push_str(&format!(" COLLATE \"{}\"", new_collation));
            }
            alter_sql.push(';');
            statements.push(alter_sql);
        } else if let Some((_, Some(new_collation))) = &mod_col.changes.collation_change {
             // Collation changed but Type didn't. Must use SET DATA TYPE ... COLLATE
             if let Some(col) = local_table.columns.get(col_name) {
                 statements.push(format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET DATA TYPE {} COLLATE \"{}\";",
                    table_name, col_name, col.data_type, new_collation
                ));
             }
        }

        // Nullability
        if let Some((_, to_nullable)) = mod_col.changes.nullable_change {
            if to_nullable {
                statements.push(format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP NOT NULL;",
                    table_name, col_name
                ));
            } else {
                statements.push(format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET NOT NULL;",
                    table_name, col_name
                ));
            }
        }

        // Default
        if let Some((_, new_default)) = &mod_col.changes.default_change {
            if let Some(def) = new_default {
                statements.push(format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET DEFAULT {};",
                    table_name, col_name, def
                ));
            } else {
                statements.push(format!(
                    "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP DEFAULT;",
                    table_name, col_name
                ));
            }
        }

        // Identity
        if let Some((old_identity, new_identity)) = &mod_col.changes.identity_change {
            match (old_identity, new_identity) {
                (Some(_), None) => {
                    statements.push(format!(
                        "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" DROP IDENTITY;",
                        table_name, col_name
                    ));
                }
                (None, Some(new_id)) => {
                    statements.push(format!(
                        "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" ADD GENERATED {} AS IDENTITY;",
                        table_name, col_name, new_id
                    ));
                }
                (Some(_), Some(new_id)) => {
                     statements.push(format!(
                        "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" SET GENERATED {};",
                        table_name, col_name, new_id
                    ));
                }
                (None, None) => {}
            }
        }
    }

    // RLS changes
    if let Some(enable) = diff.rls_change {
        if enable {
            statements.push(format!(
                "ALTER TABLE \"{}\" ENABLE ROW LEVEL SECURITY;",
                table_name
            ));
        } else {
            statements.push(format!(
                "ALTER TABLE \"{}\" DISABLE ROW LEVEL SECURITY;",
                table_name
            ));
        }
    }

    // Add check constraints
    for check in &diff.check_constraints_to_create {
        statements.push(format!(
            "ALTER TABLE \"{}\" ADD CONSTRAINT \"{}\" {};",
            table_name, check.name, check.expression
        ));
    }

    // Create indexes
    for i in &diff.indexes_to_create {
        if i.owning_constraint.is_some() {
            // Unique constraint
            let cols: Vec<String> = i.columns.iter().map(|c| format!("\"{}\"", c)).collect();
            statements.push(format!(
                "ALTER TABLE \"{}\" ADD CONSTRAINT \"{}\" UNIQUE ({});",
                table_name,
                i.index_name,
                cols.join(", ")
            ));
        } else {
            statements.push(generate_create_index(table_name, i));
        }
    }

    // Create triggers
    for t in &diff.triggers_to_create {
        statements.push(generate_create_trigger(table_name, t));
    }

    // Create policies
    for p in &diff.policies_to_create {
        statements.push(generate_create_policy(table_name, p));
    }

    // Foreign keys are handled separately in generate_sql to ensure proper ordering

    statements
}

fn escape_string(s: &str) -> String {
    s.replace('\'', "''")
}

fn generate_create_role(role: &RoleInfo) -> String {
    let mut sql = format!("CREATE ROLE \"{}\"", role.name);

    let mut options = Vec::new();

    if role.superuser { options.push("SUPERUSER"); } else { options.push("NOSUPERUSER"); }
    if role.create_db { options.push("CREATEDB"); } else { options.push("NOCREATEDB"); }
    if role.create_role { options.push("CREATEROLE"); } else { options.push("NOCREATEROLE"); }
    if role.inherit { options.push("INHERIT"); } else { options.push("NOINHERIT"); }
    if role.login { options.push("LOGIN"); } else { options.push("NOLOGIN"); }
    if role.replication { options.push("REPLICATION"); } else { options.push("NOREPLICATION"); }
    if role.bypass_rls { options.push("BYPASSRLS"); } else { options.push("NOBYPASSRLS"); }

    if role.connection_limit != -1 {
         options.push("CONNECTION LIMIT");
    }

    let mut option_str = options.join(" ");

    if role.connection_limit != -1 {
         option_str = option_str.replace("CONNECTION LIMIT", &format!("CONNECTION LIMIT {}", role.connection_limit));
    }

    if let Some(valid) = &role.valid_until {
        option_str.push_str(&format!(" VALID UNTIL '{}'", valid));
    }

    if let Some(pwd) = &role.password {
         option_str.push_str(&format!(" PASSWORD '{}'", pwd));
    }

    if !option_str.is_empty() {
        sql.push_str(" WITH ");
        sql.push_str(&option_str);
    }

    sql.push(';');
    sql
}

fn generate_alter_role(role: &RoleInfo) -> String {
    let mut sql = format!("ALTER ROLE \"{}\"", role.name);
    let mut options = Vec::new();

    if role.superuser { options.push("SUPERUSER"); } else { options.push("NOSUPERUSER"); }
    if role.create_db { options.push("CREATEDB"); } else { options.push("NOCREATEDB"); }
    if role.create_role { options.push("CREATEROLE"); } else { options.push("NOCREATEROLE"); }
    if role.inherit { options.push("INHERIT"); } else { options.push("NOINHERIT"); }
    if role.login { options.push("LOGIN"); } else { options.push("NOLOGIN"); }
    if role.replication { options.push("REPLICATION"); } else { options.push("NOREPLICATION"); }
    if role.bypass_rls { options.push("BYPASSRLS"); } else { options.push("NOBYPASSRLS"); }

    let mut option_str = options.join(" ");

     if role.connection_limit != -1 {
         option_str.push_str(&format!(" CONNECTION LIMIT {}", role.connection_limit));
    }

    if let Some(valid) = &role.valid_until {
        option_str.push_str(&format!(" VALID UNTIL '{}'", valid));
    }

     if let Some(pwd) = &role.password {
         option_str.push_str(&format!(" PASSWORD '{}'", pwd));
    }

    if !option_str.is_empty() {
        sql.push_str(" WITH ");
        sql.push_str(&option_str);
    }

    sql.push(';');
    sql
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::*;
    use crate::schema::*;
    use std::collections::HashMap;

    #[test]
    fn test_generate_sql_full() {
        // Setup diff
        let diff = SchemaDiff {
            tables_to_create: vec![],
            tables_to_drop: vec![],
            table_changes: HashMap::new(),
            enum_changes: vec![],
            functions_to_create: vec![FunctionInfo {
                name: "new_func".to_string(),
                args: vec![FunctionArg {
                    name: "a".to_string(),
                    type_: "int".to_string(),
                    mode: None,
                    default_value: None,
                }],
                return_type: "void".to_string(),
                language: "plpgsql".to_string(),
                definition: "BEGIN END;".to_string(),
                volatility: None,
                is_strict: false,
                security_definer: false,
            }],
            functions_to_drop: vec!["old_func".to_string()],
            functions_to_update: vec![],
            views_to_create: vec![],
            views_to_drop: vec![],
            views_to_update: vec![],
            sequences_to_create: vec![],
            sequences_to_drop: vec![],
            sequences_to_update: vec![],
            extensions_to_create: vec![],
            extensions_to_drop: vec![],
            composite_types_to_create: vec![],
            composite_types_to_drop: vec![],
            domains_to_create: vec![],
            domains_to_drop: vec![],
        };

        // Run generator
        let schema = DbSchema::new();
        let sql = generate_sql(&diff, &schema);

        assert!(sql.contains("CREATE OR REPLACE FUNCTION new_func"));
        assert!(sql.contains("DROP FUNCTION IF EXISTS \"old_func\""));
    }

    #[test]
    fn test_generate_create_index_with_method_and_where() {
        let idx = IndexInfo {
            index_name: "idx_active_users".to_string(),
            columns: vec!["email".to_string()],
            is_unique: true,
            is_primary: false,
            owning_constraint: None,
            index_method: "gin".to_string(),
            where_clause: Some("active = true".to_string()),
            expressions: vec![],
        };

        let sql = generate_create_index("users", &idx);
        assert!(sql.contains("USING gin"));
        assert!(sql.contains("WHERE active = true"));
    }

    #[test]
    fn test_generate_trigger_with_when() {
        let trigger = TriggerInfo {
            name: "notify_changes".to_string(),
            events: vec!["UPDATE".to_string()],
            timing: "AFTER".to_string(),
            orientation: "ROW".to_string(),
            function_name: "notify_trigger".to_string(),
            when_clause: Some("OLD.status IS DISTINCT FROM NEW.status".to_string()),
        };

        let sql = generate_create_trigger("users", &trigger);
        assert!(sql.contains("WHEN (OLD.status IS DISTINCT FROM NEW.status)"));
    }

    #[test]
    fn test_generate_foreign_key_with_on_update() {
        let fk = ForeignKeyInfo {
            constraint_name: "fk_user_org".to_string(),
            column_name: "org_id".to_string(),
            foreign_table: "organizations".to_string(),
            foreign_column: "id".to_string(),
            on_delete: "CASCADE".to_string(),
            on_update: "SET NULL".to_string(),
        };

        let sql = generate_add_foreign_key("users", &fk);
        assert!(sql.contains("ON DELETE CASCADE"));
        assert!(sql.contains("ON UPDATE SET NULL"));
    }

    #[test]
    fn test_generate_create_sequence() {
        let seq = SequenceInfo {
            name: "user_id_seq".to_string(),
            data_type: "bigint".to_string(),
            start_value: 1,
            min_value: 1,
            max_value: 1000000,
            increment: 1,
            cycle: false,
            cache_size: 10,
            owned_by: Some("users.id".to_string()),
            comment: None,
        };

        let sql = generate_create_sequence(&seq);
        assert!(sql.contains("CREATE SEQUENCE"));
        assert!(sql.contains("CACHE 10"));
        assert!(sql.contains("OWNED BY users.id"));
    }

    #[test]
    fn test_generate_create_view() {
        let view = ViewInfo {
            name: "active_users".to_string(),
            definition: "SELECT * FROM users WHERE active = true".to_string(),
            is_materialized: false,
            columns: vec![],
            indexes: vec![],
            comment: None,
            with_options: vec!["security_barrier=true".to_string()],
            check_option: None,
        };

        let sql = generate_create_view(&view);
        assert!(sql.contains("CREATE OR REPLACE VIEW"));
        assert!(sql.contains("WITH (security_barrier=true)"));
    }

    #[test]
    fn test_generate_materialized_view() {
        let view = ViewInfo {
            name: "user_stats".to_string(),
            definition: "SELECT user_id, count(*) FROM posts GROUP BY user_id".to_string(),
            is_materialized: true,
            columns: vec![],
            indexes: vec![],
            comment: None,
            with_options: vec![],
            check_option: None,
        };

        let sql = generate_create_view(&view);
        assert!(sql.contains("CREATE MATERIALIZED VIEW"));
    }

    #[test]
    fn test_generate_create_domain() {
        let domain = DomainInfo {
            name: "email_address".to_string(),
            base_type: "text".to_string(),
            default_value: None,
            is_not_null: true,
            check_constraints: vec![DomainCheckConstraint {
                name: Some("valid_email".to_string()),
                expression: "CHECK (VALUE ~ '^[^@]+@[^@]+$')".to_string(),
            }],
            collation: None,
            comment: None,
        };

        let sql = generate_create_domain(&domain);
        assert!(sql.contains("CREATE DOMAIN"));
        assert!(sql.contains("NOT NULL"));
        assert!(sql.contains("CONSTRAINT \"valid_email\""));
    }

    #[test]
    fn test_generate_composite_type() {
        let comp_type = CompositeTypeInfo {
            name: "address".to_string(),
            attributes: vec![
                CompositeTypeAttribute {
                    name: "street".to_string(),
                    data_type: "text".to_string(),
                    collation: None,
                },
                CompositeTypeAttribute {
                    name: "city".to_string(),
                    data_type: "text".to_string(),
                    collation: None,
                },
            ],
            comment: None,
        };

        let sql = generate_create_composite_type(&comp_type);
        assert!(sql.contains("CREATE TYPE \"address\" AS"));
        assert!(sql.contains("\"street\" text"));
        assert!(sql.contains("\"city\" text"));
    }

    #[test]
    fn test_generate_extension() {
        let ext = ExtensionInfo {
            name: "uuid-ossp".to_string(),
            version: Some("1.1".to_string()),
            schema: Some("extensions".to_string()),
        };

        let sql = generate_create_extension(&ext);
        assert!(sql.contains("CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\""));
        assert!(sql.contains("WITH SCHEMA \"extensions\""));
        assert!(sql.contains("VERSION '1.1'"));
    }
}
