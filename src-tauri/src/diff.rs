use crate::schema::{
    CheckConstraintInfo, CompositeTypeInfo, DbSchema, DomainInfo, EnumInfo, ExtensionInfo,
    ForeignKeyInfo, FunctionInfo, IndexInfo, PolicyInfo, SequenceInfo, TableInfo, TriggerInfo,
    ViewInfo,
};
use std::collections::HashMap;

#[derive(Debug)]
pub struct SchemaDiff {
    pub tables_to_create: Vec<String>,
    pub tables_to_drop: Vec<String>,
    pub table_changes: HashMap<String, TableDiff>,
    pub enum_changes: Vec<EnumChange>,
    pub functions_to_create: Vec<FunctionInfo>,
    pub functions_to_drop: Vec<String>,
    pub functions_to_update: Vec<FunctionInfo>,
    // New entity diffs
    pub views_to_create: Vec<ViewInfo>,
    pub views_to_drop: Vec<String>,
    pub views_to_update: Vec<ViewInfo>,
    pub sequences_to_create: Vec<SequenceInfo>,
    pub sequences_to_drop: Vec<String>,
    pub sequences_to_update: Vec<SequenceInfo>,
    pub extensions_to_create: Vec<ExtensionInfo>,
    pub extensions_to_drop: Vec<String>,
    pub composite_types_to_create: Vec<CompositeTypeInfo>,
    pub composite_types_to_drop: Vec<String>,
    pub domains_to_create: Vec<DomainInfo>,
    pub domains_to_drop: Vec<String>,
}

#[derive(Debug)]
pub struct TableDiff {
    pub columns_to_add: Vec<String>,
    pub columns_to_drop: Vec<String>,
    pub columns_to_modify: Vec<ColumnModification>,
    pub rls_change: Option<bool>,
    pub policies_to_create: Vec<PolicyInfo>,
    pub policies_to_drop: Vec<PolicyInfo>,
    pub triggers_to_create: Vec<TriggerInfo>,
    pub triggers_to_drop: Vec<TriggerInfo>,
    pub indexes_to_create: Vec<IndexInfo>,
    pub indexes_to_drop: Vec<IndexInfo>,
    pub check_constraints_to_create: Vec<CheckConstraintInfo>,
    pub check_constraints_to_drop: Vec<CheckConstraintInfo>,
    pub foreign_keys_to_create: Vec<ForeignKeyInfo>,
    pub foreign_keys_to_drop: Vec<ForeignKeyInfo>,
    pub comment_change: Option<Option<String>>,
}

#[derive(Debug)]
pub struct ColumnModification {
    pub column_name: String,
    pub changes: ColumnChangeDetail,
}

#[derive(Debug)]
pub struct ColumnChangeDetail {
    pub type_change: Option<(String, String)>,
    pub nullable_change: Option<(bool, bool)>,
    pub default_change: Option<(Option<String>, Option<String>)>,
    pub comment_change: Option<(Option<String>, Option<String>)>,
}

#[derive(Debug)]
pub struct EnumChange {
    pub type_: EnumChangeType,
    pub name: String,
    pub values: Option<Vec<String>>,
    pub values_to_add: Option<Vec<String>>,
}

#[derive(Debug, PartialEq)]
pub enum EnumChangeType {
    Create,
    Drop,
    AddValue,
}

pub fn compute_diff(remote: &DbSchema, local: &DbSchema) -> SchemaDiff {
    let mut diff = SchemaDiff {
        tables_to_create: vec![],
        tables_to_drop: vec![],
        table_changes: HashMap::new(),
        enum_changes: vec![],
        functions_to_create: vec![],
        functions_to_drop: vec![],
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

    // 1. Tables
    for (name, _) in &local.tables {
        if !remote.tables.contains_key(name) {
            diff.tables_to_create.push(name.clone());
        }
    }

    for (name, _) in &remote.tables {
        if !local.tables.contains_key(name) {
            diff.tables_to_drop.push(name.clone());
        }
    }

    // Table modifications
    for (name, local_table) in &local.tables {
        if let Some(remote_table) = remote.tables.get(name) {
            let table_diff = compute_table_diff(remote_table, local_table);
            if !table_diff.is_empty() {
                diff.table_changes.insert(name.clone(), table_diff);
            }
        }
    }

    // 2. Enums (with proper AddValue support)
    for (name, local_info) in &local.enums {
        if let Some(remote_info) = remote.enums.get(name) {
            // Check for added values
            let new_values: Vec<String> = local_info
                .values
                .iter()
                .filter(|v| !remote_info.values.contains(v))
                .cloned()
                .collect();

            if !new_values.is_empty() {
                diff.enum_changes.push(EnumChange {
                    type_: EnumChangeType::AddValue,
                    name: name.clone(),
                    values: Some(local_info.values.clone()),
                    values_to_add: Some(new_values),
                });
            }
        } else {
            diff.enum_changes.push(EnumChange {
                type_: EnumChangeType::Create,
                name: name.clone(),
                values: Some(local_info.values.clone()),
                values_to_add: None,
            });
        }
    }

    for (name, _) in &remote.enums {
        if !local.enums.contains_key(name) {
            diff.enum_changes.push(EnumChange {
                type_: EnumChangeType::Drop,
                name: name.clone(),
                values: None,
                values_to_add: None,
            });
        }
    }

    // 3. Functions
    for (name, local_func) in &local.functions {
        if let Some(remote_func) = remote.functions.get(name) {
            if local_func != remote_func {
                diff.functions_to_update.push(local_func.clone());
            }
        } else {
            diff.functions_to_create.push(local_func.clone());
        }
    }

    for (name, _) in &remote.functions {
        if !local.functions.contains_key(name) {
            diff.functions_to_drop.push(name.clone());
        }
    }

    // 4. Views
    for (name, local_view) in &local.views {
        if let Some(remote_view) = remote.views.get(name) {
            if views_differ(local_view, remote_view) {
                diff.views_to_update.push(local_view.clone());
            }
        } else {
            diff.views_to_create.push(local_view.clone());
        }
    }

    for (name, _) in &remote.views {
        if !local.views.contains_key(name) {
            diff.views_to_drop.push(name.clone());
        }
    }

    // 5. Sequences
    for (name, local_seq) in &local.sequences {
        if let Some(remote_seq) = remote.sequences.get(name) {
            if sequences_differ(local_seq, remote_seq) {
                diff.sequences_to_update.push(local_seq.clone());
            }
        } else {
            diff.sequences_to_create.push(local_seq.clone());
        }
    }

    for (name, _) in &remote.sequences {
        if !local.sequences.contains_key(name) {
            diff.sequences_to_drop.push(name.clone());
        }
    }

    // 6. Extensions
    for (name, local_ext) in &local.extensions {
        if !remote.extensions.contains_key(name) {
            diff.extensions_to_create.push(local_ext.clone());
        }
    }

    for (name, _) in &remote.extensions {
        if !local.extensions.contains_key(name) {
            diff.extensions_to_drop.push(name.clone());
        }
    }

    // 7. Composite Types
    for (name, local_type) in &local.composite_types {
        if !remote.composite_types.contains_key(name) {
            diff.composite_types_to_create.push(local_type.clone());
        }
        // Note: Modifying composite types is complex (requires DROP + CREATE)
        // For now, we only handle create/drop
    }

    for (name, _) in &remote.composite_types {
        if !local.composite_types.contains_key(name) {
            diff.composite_types_to_drop.push(name.clone());
        }
    }

    // 8. Domains
    for (name, local_domain) in &local.domains {
        if !remote.domains.contains_key(name) {
            diff.domains_to_create.push(local_domain.clone());
        }
    }

    for (name, _) in &remote.domains {
        if !local.domains.contains_key(name) {
            diff.domains_to_drop.push(name.clone());
        }
    }

    diff
}

fn views_differ(local: &ViewInfo, remote: &ViewInfo) -> bool {
    // Normalize definitions for comparison (remove extra whitespace)
    let local_def = normalize_sql(&local.definition);
    let remote_def = normalize_sql(&remote.definition);

    local_def != remote_def
        || local.is_materialized != remote.is_materialized
        || local.with_options != remote.with_options
}

fn sequences_differ(local: &SequenceInfo, remote: &SequenceInfo) -> bool {
    local.data_type != remote.data_type
        || local.increment != remote.increment
        || local.min_value != remote.min_value
        || local.max_value != remote.max_value
        || local.cycle != remote.cycle
        || local.cache_size != remote.cache_size
}

fn normalize_sql(sql: &str) -> String {
    sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn compute_table_diff(remote: &TableInfo, local: &TableInfo) -> TableDiff {
    let mut diff = TableDiff {
        columns_to_add: vec![],
        columns_to_drop: vec![],
        columns_to_modify: vec![],
        rls_change: None,
        policies_to_create: vec![],
        policies_to_drop: vec![],
        triggers_to_create: vec![],
        triggers_to_drop: vec![],
        indexes_to_create: vec![],
        indexes_to_drop: vec![],
        check_constraints_to_create: vec![],
        check_constraints_to_drop: vec![],
        foreign_keys_to_create: vec![],
        foreign_keys_to_drop: vec![],
        comment_change: None,
    };

    // Columns
    for (name, _) in &local.columns {
        if !remote.columns.contains_key(name) {
            diff.columns_to_add.push(name.clone());
        }
    }

    for (name, _) in &remote.columns {
        if !local.columns.contains_key(name) {
            diff.columns_to_drop.push(name.clone());
        }
    }

    // Column Modifications
    for (name, local_col) in &local.columns {
        if let Some(remote_col) = remote.columns.get(name) {
            let mut changes = ColumnChangeDetail {
                type_change: None,
                nullable_change: None,
                default_change: None,
                comment_change: None,
            };

            // Type comparison (case-insensitive)
            if local_col.data_type.to_lowercase() != remote_col.data_type.to_lowercase() {
                changes.type_change =
                    Some((remote_col.data_type.clone(), local_col.data_type.clone()));
            }

            // Nullability
            if local_col.is_nullable != remote_col.is_nullable {
                changes.nullable_change = Some((remote_col.is_nullable, local_col.is_nullable));
            }

            // Default value
            if local_col.column_default != remote_col.column_default {
                changes.default_change = Some((
                    remote_col.column_default.clone(),
                    local_col.column_default.clone(),
                ));
            }

            // Comment
            if local_col.comment != remote_col.comment {
                changes.comment_change =
                    Some((remote_col.comment.clone(), local_col.comment.clone()));
            }

            if changes.type_change.is_some()
                || changes.nullable_change.is_some()
                || changes.default_change.is_some()
                || changes.comment_change.is_some()
            {
                diff.columns_to_modify.push(ColumnModification {
                    column_name: name.clone(),
                    changes,
                });
            }
        }
    }

    // RLS Status
    if local.rls_enabled != remote.rls_enabled {
        diff.rls_change = Some(local.rls_enabled);
    }

    // Table comment
    if local.comment != remote.comment {
        diff.comment_change = Some(local.comment.clone());
    }

    // Policies
    let remote_policies: HashMap<&String, &PolicyInfo> =
        remote.policies.iter().map(|p| (&p.name, p)).collect();
    let local_policies: HashMap<&String, &PolicyInfo> =
        local.policies.iter().map(|p| (&p.name, p)).collect();

    for p in &local.policies {
        if !remote_policies.contains_key(&p.name) {
            diff.policies_to_create.push(p.clone());
        } else {
            let remote_p = remote_policies.get(&p.name).unwrap();
            if p != *remote_p {
                diff.policies_to_drop.push((*remote_p).clone());
                diff.policies_to_create.push(p.clone());
            }
        }
    }

    for p in &remote.policies {
        if !local_policies.contains_key(&p.name) {
            diff.policies_to_drop.push(p.clone());
        }
    }

    // Triggers (including WHEN clause comparison)
    let remote_triggers: HashMap<&String, &TriggerInfo> =
        remote.triggers.iter().map(|t| (&t.name, t)).collect();
    let local_triggers: HashMap<&String, &TriggerInfo> =
        local.triggers.iter().map(|t| (&t.name, t)).collect();

    for t in &local.triggers {
        if !remote_triggers.contains_key(&t.name) {
            diff.triggers_to_create.push(t.clone());
        } else {
            let remote_t = remote_triggers.get(&t.name).unwrap();
            if triggers_differ(t, remote_t) {
                diff.triggers_to_drop.push((*remote_t).clone());
                diff.triggers_to_create.push(t.clone());
            }
        }
    }

    for t in &remote.triggers {
        if !local_triggers.contains_key(&t.name) {
            diff.triggers_to_drop.push(t.clone());
        }
    }

    // Indexes (including method and where clause comparison)
    let remote_indexes: HashMap<&String, &IndexInfo> =
        remote.indexes.iter().map(|i| (&i.index_name, i)).collect();
    let local_indexes: HashMap<&String, &IndexInfo> =
        local.indexes.iter().map(|i| (&i.index_name, i)).collect();

    for i in &local.indexes {
        if !remote_indexes.contains_key(&i.index_name) {
            diff.indexes_to_create.push(i.clone());
        } else {
            let remote_i = remote_indexes.get(&i.index_name).unwrap();
            if indexes_differ(i, remote_i) {
                diff.indexes_to_drop.push((*remote_i).clone());
                diff.indexes_to_create.push(i.clone());
            }
        }
    }

    for i in &remote.indexes {
        if !local_indexes.contains_key(&i.index_name) {
            diff.indexes_to_drop.push(i.clone());
        }
    }

    // Check Constraints
    let remote_checks: HashMap<&String, &CheckConstraintInfo> = remote
        .check_constraints
        .iter()
        .map(|c| (&c.name, c))
        .collect();
    let local_checks: HashMap<&String, &CheckConstraintInfo> = local
        .check_constraints
        .iter()
        .map(|c| (&c.name, c))
        .collect();

    for c in &local.check_constraints {
        if !remote_checks.contains_key(&c.name) {
            diff.check_constraints_to_create.push(c.clone());
        } else {
            let remote_c = remote_checks.get(&c.name).unwrap();
            // Normalize expressions for comparison
            if normalize_sql(&c.expression) != normalize_sql(&remote_c.expression) {
                diff.check_constraints_to_drop.push((*remote_c).clone());
                diff.check_constraints_to_create.push(c.clone());
            }
        }
    }

    for c in &remote.check_constraints {
        if !local_checks.contains_key(&c.name) {
            diff.check_constraints_to_drop.push(c.clone());
        }
    }

    // Foreign Keys (including ON UPDATE comparison)
    let remote_fks: HashMap<&String, &ForeignKeyInfo> = remote
        .foreign_keys
        .iter()
        .map(|f| (&f.constraint_name, f))
        .collect();
    let local_fks: HashMap<&String, &ForeignKeyInfo> = local
        .foreign_keys
        .iter()
        .map(|f| (&f.constraint_name, f))
        .collect();

    for f in &local.foreign_keys {
        if !remote_fks.contains_key(&f.constraint_name) {
            diff.foreign_keys_to_create.push(f.clone());
        } else {
            let remote_f = remote_fks.get(&f.constraint_name).unwrap();
            if foreign_keys_differ(f, remote_f) {
                diff.foreign_keys_to_drop.push((*remote_f).clone());
                diff.foreign_keys_to_create.push(f.clone());
            }
        }
    }

    for f in &remote.foreign_keys {
        if !local_fks.contains_key(&f.constraint_name) {
            diff.foreign_keys_to_drop.push(f.clone());
        }
    }

    diff
}

fn triggers_differ(local: &TriggerInfo, remote: &TriggerInfo) -> bool {
    local.events != remote.events
        || local.timing != remote.timing
        || local.orientation != remote.orientation
        || local.function_name != remote.function_name
        || local.when_clause != remote.when_clause
}

fn indexes_differ(local: &IndexInfo, remote: &IndexInfo) -> bool {
    local.columns != remote.columns
        || local.is_unique != remote.is_unique
        || local.is_primary != remote.is_primary
        || local.index_method.to_lowercase() != remote.index_method.to_lowercase()
        || normalize_option(&local.where_clause) != normalize_option(&remote.where_clause)
        || local.expressions != remote.expressions
}

fn foreign_keys_differ(local: &ForeignKeyInfo, remote: &ForeignKeyInfo) -> bool {
    local.column_name != remote.column_name
        || local.foreign_table != remote.foreign_table
        || local.foreign_column != remote.foreign_column
        || local.on_delete != remote.on_delete
        || local.on_update != remote.on_update
}

fn normalize_option(opt: &Option<String>) -> Option<String> {
    opt.as_ref().map(|s| normalize_sql(s))
}

impl TableDiff {
    pub fn is_empty(&self) -> bool {
        self.columns_to_add.is_empty()
            && self.columns_to_drop.is_empty()
            && self.columns_to_modify.is_empty()
            && self.rls_change.is_none()
            && self.policies_to_create.is_empty()
            && self.policies_to_drop.is_empty()
            && self.triggers_to_create.is_empty()
            && self.triggers_to_drop.is_empty()
            && self.indexes_to_create.is_empty()
            && self.indexes_to_drop.is_empty()
            && self.check_constraints_to_create.is_empty()
            && self.check_constraints_to_drop.is_empty()
            && self.foreign_keys_to_create.is_empty()
            && self.foreign_keys_to_drop.is_empty()
            && self.comment_change.is_none()
    }
}

impl SchemaDiff {
    pub fn is_empty(&self) -> bool {
        self.tables_to_create.is_empty()
            && self.tables_to_drop.is_empty()
            && self.table_changes.is_empty()
            && self.enum_changes.is_empty()
            && self.functions_to_create.is_empty()
            && self.functions_to_drop.is_empty()
            && self.functions_to_update.is_empty()
            && self.views_to_create.is_empty()
            && self.views_to_drop.is_empty()
            && self.views_to_update.is_empty()
            && self.sequences_to_create.is_empty()
            && self.sequences_to_drop.is_empty()
            && self.sequences_to_update.is_empty()
            && self.extensions_to_create.is_empty()
            && self.extensions_to_drop.is_empty()
            && self.composite_types_to_create.is_empty()
            && self.composite_types_to_drop.is_empty()
            && self.domains_to_create.is_empty()
            && self.domains_to_drop.is_empty()
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize() {
        let diff = SchemaDiff {
            tables_to_create: vec!["users".to_string()],
            tables_to_drop: vec!["posts".to_string()],
            table_changes: HashMap::new(),
            enum_changes: vec![],
            functions_to_create: vec![],
            functions_to_drop: vec![],
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

        let summary = diff.summarize();
        assert!(summary.contains("+ Table 'users'"));
        assert!(summary.contains("- Table 'posts'"));
    }

    #[test]
    fn test_enum_add_value() {
        use crate::schema::*;

        let mut remote = DbSchema::new();
        remote.enums.insert(
            "status".to_string(),
            EnumInfo {
                name: "status".to_string(),
                values: vec!["active".to_string(), "inactive".to_string()],
            },
        );

        let mut local = DbSchema::new();
        local.enums.insert(
            "status".to_string(),
            EnumInfo {
                name: "status".to_string(),
                values: vec![
                    "active".to_string(),
                    "inactive".to_string(),
                    "pending".to_string(),
                ],
            },
        );

        let diff = compute_diff(&remote, &local);

        assert_eq!(diff.enum_changes.len(), 1);
        assert_eq!(diff.enum_changes[0].type_, EnumChangeType::AddValue);
        assert_eq!(
            diff.enum_changes[0].values_to_add,
            Some(vec!["pending".to_string()])
        );
    }

    #[test]
    fn test_index_method_comparison() {
        let local = IndexInfo {
            index_name: "idx_test".to_string(),
            columns: vec!["col1".to_string()],
            is_unique: false,
            is_primary: false,
            owning_constraint: None,
            index_method: "gin".to_string(),
            where_clause: None,
            expressions: vec![],
        };

        let remote = IndexInfo {
            index_name: "idx_test".to_string(),
            columns: vec!["col1".to_string()],
            is_unique: false,
            is_primary: false,
            owning_constraint: None,
            index_method: "btree".to_string(),
            where_clause: None,
            expressions: vec![],
        };

        assert!(indexes_differ(&local, &remote));
    }

    #[test]
    fn test_trigger_when_clause_comparison() {
        let local = TriggerInfo {
            name: "trig_test".to_string(),
            events: vec!["UPDATE".to_string()],
            timing: "AFTER".to_string(),
            orientation: "ROW".to_string(),
            function_name: "notify".to_string(),
            when_clause: Some("OLD.status <> NEW.status".to_string()),
        };

        let remote = TriggerInfo {
            name: "trig_test".to_string(),
            events: vec!["UPDATE".to_string()],
            timing: "AFTER".to_string(),
            orientation: "ROW".to_string(),
            function_name: "notify".to_string(),
            when_clause: None,
        };

        assert!(triggers_differ(&local, &remote));
    }

    #[test]
    fn test_foreign_key_on_update_comparison() {
        let local = ForeignKeyInfo {
            constraint_name: "fk_test".to_string(),
            column_name: "user_id".to_string(),
            foreign_table: "users".to_string(),
            foreign_column: "id".to_string(),
            on_delete: "CASCADE".to_string(),
            on_update: "SET NULL".to_string(),
        };

        let remote = ForeignKeyInfo {
            constraint_name: "fk_test".to_string(),
            column_name: "user_id".to_string(),
            foreign_table: "users".to_string(),
            foreign_column: "id".to_string(),
            on_delete: "CASCADE".to_string(),
            on_update: "NO ACTION".to_string(),
        };

        assert!(foreign_keys_differ(&local, &remote));
    }
}
