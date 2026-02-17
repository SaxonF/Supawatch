use crate::diff::TableDiff;
use crate::schema::TableInfo;
use super::constraints::generate_create_index;

pub fn generate_create_table(table: &TableInfo) -> String {
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

    for col in &columns {
        let mut col_sql = format!("\"{}\" {}", col.column_name, col.data_type);

        if let Some(collation) = &col.collation {
            col_sql.push_str(&format!(" COLLATE \"{}\"", collation));
        }

        if !col.is_nullable && !col.is_primary_key {
            col_sql.push_str(" NOT NULL");
        }

        // Generated columns use GENERATED ALWAYS AS ... STORED (mutually exclusive with DEFAULT)
        if col.is_generated {
            if let Some(expr) = &col.generation_expression {
                col_sql.push_str(&format!(" GENERATED ALWAYS AS ({}) STORED", expr));
            }
        } else if let Some(def) = &col.column_default {
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

    let qualified_name = format!("\"{}\".\"{}\"", table.schema, table.table_name);

    let mut sql = format!(
        "CREATE TABLE {} (\n  {}\n);",
        qualified_name,
        col_defs.join(",\n  ")
    );

    // Indexes (non-primary)
    for idx in &table.indexes {
        if !idx.is_primary {
            sql.push('\n');
            // Pass qualified name to generate_create_index
            sql.push_str(&generate_create_index(&qualified_name, idx));
        }
    }

    // RLS
    if table.rls_enabled {
        sql.push_str(&format!(
            "\nALTER TABLE {} ENABLE ROW LEVEL SECURITY;",
            qualified_name
        ));
    }

    sql
}

pub fn generate_alter_table(
    table_name: &str,
    diff: &TableDiff,
    local_table: &TableInfo,
) -> Vec<String> {
    let mut statements = vec![];

    // Drop foreign keys first (before dropping columns they reference)
    for fk in &diff.foreign_keys_to_drop {
        statements.push(format!(
            "ALTER TABLE {} DROP CONSTRAINT IF EXISTS \"{}\";",
            table_name, fk.constraint_name
        ));
    }

    // Drop check constraints
    for check in &diff.check_constraints_to_drop {
        statements.push(format!(
            "ALTER TABLE {} DROP CONSTRAINT IF EXISTS \"{}\";",
            table_name, check.name
        ));
    }

    // Drop policies
    for p in &diff.policies_to_drop {
        statements.push(format!(
            "DROP POLICY IF EXISTS \"{}\" ON {};",
            p.name, table_name
        ));
    }

    // Drop triggers
    for t in &diff.triggers_to_drop {
        statements.push(format!(
            "DROP TRIGGER IF EXISTS \"{}\" ON {};",
            t.name, table_name
        ));
    }

    // Drop indexes
    for i in &diff.indexes_to_drop {
        if let Some(constraint) = &i.owning_constraint {
            statements.push(format!(
                "ALTER TABLE {} DROP CONSTRAINT IF EXISTS \"{}\";",
                table_name, constraint
            ));
        } else {
            statements.push(format!("DROP INDEX IF EXISTS \"{}\".\"{}\";", local_table.schema, i.index_name));
        }
    }

    // Drop columns
    for col in &diff.columns_to_drop {
        statements.push(format!(
            "ALTER TABLE {} DROP COLUMN IF EXISTS \"{}\";",
            table_name, col
        ));
    }

    // Add columns (non-generated first)
    for col_name in &diff.columns_to_add {
        if let Some(col) = local_table.columns.get(col_name) {
            if col.is_generated {
                continue;
            }

            let mut add_sql = format!(
                "ALTER TABLE {} ADD COLUMN \"{}\" {}",
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
                "ALTER TABLE {} ALTER COLUMN \"{}\" TYPE {} USING \"{}\"::{}",
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
                    "ALTER TABLE {} ALTER COLUMN \"{}\" SET DATA TYPE {} COLLATE \"{}\";",
                    table_name, col_name, col.data_type, new_collation
                ));
             }
        }

        // Nullability
        if let Some((_, to_nullable)) = mod_col.changes.nullable_change {
            if to_nullable {
                statements.push(format!(
                    "ALTER TABLE {} ALTER COLUMN \"{}\" DROP NOT NULL;",
                    table_name, col_name
                ));
            } else {
                statements.push(format!(
                    "ALTER TABLE {} ALTER COLUMN \"{}\" SET NOT NULL;",
                    table_name, col_name
                ));
            }
        }

        // Default
        if let Some((_, new_default)) = &mod_col.changes.default_change {
            if let Some(def) = new_default {
                statements.push(format!(
                    "ALTER TABLE {} ALTER COLUMN \"{}\" SET DEFAULT {};",
                    table_name, col_name, def
                ));
            } else {
                statements.push(format!(
                    "ALTER TABLE {} ALTER COLUMN \"{}\" DROP DEFAULT;",
                    table_name, col_name
                ));
            }
        }

        // Identity
        if let Some((old_identity, new_identity)) = &mod_col.changes.identity_change {
            match (old_identity, new_identity) {
                (Some(_), None) => {
                    statements.push(format!(
                        "ALTER TABLE {} ALTER COLUMN \"{}\" DROP IDENTITY;",
                        table_name, col_name
                    ));
                }
                (None, Some(new_id)) => {
                    statements.push(format!(
                        "ALTER TABLE {} ALTER COLUMN \"{}\" ADD GENERATED {} AS IDENTITY;",
                        table_name, col_name, new_id
                    ));
                }
                (Some(_), Some(new_id)) => {
                     statements.push(format!(
                        "ALTER TABLE {} ALTER COLUMN \"{}\" SET GENERATED {};",
                        table_name, col_name, new_id
                    ));
                }
                (None, None) => {}
            }
        }
    }

    // Add generated columns (after modifications, so dependencies are ready)
    for col_name in &diff.columns_to_add {
        if let Some(col) = local_table.columns.get(col_name) {
            if !col.is_generated {
                continue;
            }

            let mut add_sql = format!(
                "ALTER TABLE {} ADD COLUMN \"{}\" {}",
                table_name, col.column_name, col.data_type
            );

            if !col.is_nullable {
                add_sql.push_str(" NOT NULL");
            }

            if let Some(expr) = &col.generation_expression {
                add_sql.push_str(&format!(" GENERATED ALWAYS AS ({}) STORED", expr));
            }

            add_sql.push(';');
            statements.push(add_sql);
        }
    }

    // RLS changes
    if let Some(enable) = diff.rls_change {
        if enable {
            statements.push(format!(
                "ALTER TABLE {} ENABLE ROW LEVEL SECURITY;",
                table_name
            ));
        } else {
            statements.push(format!(
                "ALTER TABLE {} DISABLE ROW LEVEL SECURITY;",
                table_name
            ));
        }
    }

    // Add check constraints
    for check in &diff.check_constraints_to_create {
        statements.push(format!(
            "ALTER TABLE {} ADD CONSTRAINT \"{}\" {};",
            table_name, check.name, check.expression
        ));
    }

    // Create indexes
    for i in &diff.indexes_to_create {
        if i.owning_constraint.is_some() {
            // Unique constraint
            let cols: Vec<String> = i.columns.iter().map(|c| format!("\"{}\"", c)).collect();
            statements.push(format!(
                "ALTER TABLE {} ADD CONSTRAINT \"{}\" UNIQUE ({});",
                table_name,
                i.index_name,
                cols.join(", ")
            ));
        } else {
            statements.push(super::constraints::generate_create_index(table_name, i));
        }
    }

    // Create triggers
    for t in &diff.triggers_to_create {
        statements.push(super::constraints::generate_create_trigger(table_name, t));
    }

    // Create policies
    for p in &diff.policies_to_create {
        statements.push(super::constraints::generate_create_policy(table_name, p));
    }

    // Foreign keys are handled separately in generate_sql to ensure proper ordering

    statements
}
