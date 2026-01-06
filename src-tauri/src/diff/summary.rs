use super::EnumChangeType;
use crate::diff::SchemaDiff;

impl SchemaDiff {
    pub fn summarize(&self) -> String {
        let mut parts = vec![];

        // Extensions
        for ext in &self.extensions_to_create {
            parts.push(format!("+ Extension '{}'", ext.name));
        }
        for ext in &self.extensions_to_drop {
            parts.push(format!("- Extension '{}'", ext));
        }

        // Types
        for t in &self.composite_types_to_create {
            parts.push(format!("+ Type '{}'", t.name));
        }
        for t in &self.composite_types_to_drop {
            parts.push(format!("- Type '{}'", t));
        }

        // Domains
        for d in &self.domains_to_create {
            parts.push(format!("+ Domain '{}'", d.name));
        }
        for d in &self.domains_to_drop {
            parts.push(format!("- Domain '{}'", d));
        }

        // Enums
        for enum_change in &self.enum_changes {
            match enum_change.type_ {
                EnumChangeType::Create => {
                    parts.push(format!("+ Enum '{}'", enum_change.name));
                }
                EnumChangeType::Drop => {
                    parts.push(format!("- Enum '{}'", enum_change.name));
                }
                EnumChangeType::AddValue => {
                    if let Some(values) = &enum_change.values_to_add {
                        parts.push(format!(
                            "~ Enum '{}' (add values: {})",
                            enum_change.name,
                            values.join(", ")
                        ));
                    }
                }
            }
        }

        // Sequences
        for seq in &self.sequences_to_create {
            parts.push(format!("+ Sequence '{}'", seq.name));
        }
        for seq in &self.sequences_to_drop {
            parts.push(format!("- Sequence '{}'", seq));
        }
        for seq in &self.sequences_to_update {
            parts.push(format!("~ Sequence '{}'", seq.name));
        }

        // Tables
        for table in &self.tables_to_create {
            parts.push(format!("+ Table '{}'", table));
        }
        for table in &self.tables_to_drop {
            parts.push(format!("- Table '{}'", table));
        }

        // Table changes
        for (table_name, diff) in &self.table_changes {
            for col in &diff.columns_to_add {
                parts.push(format!("+ Column '{}.{}'", table_name, col));
            }
            for col in &diff.columns_to_drop {
                parts.push(format!("- Column '{}.{}'", table_name, col));
            }
            for mod_col in &diff.columns_to_modify {
                let mut changes = vec![];
                if let Some((from, to)) = &mod_col.changes.type_change {
                    changes.push(format!("type: {} -> {}", from, to));
                }
                if let Some((from, to)) = mod_col.changes.nullable_change {
                    let from_str = if from { "NULL" } else { "NOT NULL" };
                    let to_str = if to { "NULL" } else { "NOT NULL" };
                    changes.push(format!("nullable: {} -> {}", from_str, to_str));
                }
                if mod_col.changes.default_change.is_some() {
                    changes.push("default changed".to_string());
                }
                parts.push(format!(
                    "~ Column '{}.{}' ({})",
                    table_name,
                    mod_col.column_name,
                    changes.join(", ")
                ));
            }

            if let Some(rls) = diff.rls_change {
                parts.push(format!(
                    "~ Table '{}' RLS: {}",
                    table_name,
                    if rls { "ENABLE" } else { "DISABLE" }
                ));
            }

            if diff.comment_change.is_some() {
                parts.push(format!("~ Table '{}' comment changed", table_name));
            }

            for p in &diff.policies_to_create {
                parts.push(format!("+ Policy '{}' ON '{}'", p.name, table_name));
            }
            for p in &diff.policies_to_drop {
                parts.push(format!("- Policy '{}' ON '{}'", p.name, table_name));
            }

            for t in &diff.triggers_to_create {
                parts.push(format!("+ Trigger '{}' ON '{}'", t.name, table_name));
            }
            for t in &diff.triggers_to_drop {
                parts.push(format!("- Trigger '{}' ON '{}'", t.name, table_name));
            }

            for i in &diff.indexes_to_create {
                parts.push(format!("+ Index '{}' ON '{}'", i.index_name, table_name));
            }
            for i in &diff.indexes_to_drop {
                parts.push(format!("- Index '{}' ON '{}'", i.index_name, table_name));
            }

            for c in &diff.check_constraints_to_create {
                parts.push(format!("+ Check '{}' ON '{}'", c.name, table_name));
            }
            for c in &diff.check_constraints_to_drop {
                parts.push(format!("- Check '{}' ON '{}'", c.name, table_name));
            }

            for f in &diff.foreign_keys_to_create {
                parts.push(format!("+ FK '{}' ON '{}'", f.constraint_name, table_name));
            }
            for f in &diff.foreign_keys_to_drop {
                parts.push(format!("- FK '{}' ON '{}'", f.constraint_name, table_name));
            }
        }

        // Views
        for view in &self.views_to_create {
            let mat = if view.is_materialized {
                "Materialized "
            } else {
                ""
            };
            parts.push(format!("+ {}View '{}'", mat, view.name));
        }
        for view in &self.views_to_drop {
            parts.push(format!("- View '{}'", view));
        }
        for view in &self.views_to_update {
            let mat = if view.is_materialized {
                "Materialized "
            } else {
                ""
            };
            parts.push(format!("~ {}View '{}'", mat, view.name));
        }

        // Functions
        for f in &self.functions_to_create {
            parts.push(format!("+ Function '{}'", f.name));
        }
        for f in &self.functions_to_drop {
            parts.push(format!("- Function '{}'", f));
        }
        for f in &self.functions_to_update {
            parts.push(format!("~ Function '{}'", f.name));
        }

        if parts.is_empty() {
            return "No changes detected".to_string();
        }

        parts.sort();
        parts.join("\n")
    }
}
