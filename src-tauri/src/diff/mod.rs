use crate::schema::{
    CompositeTypeInfo, DbSchema, DomainInfo, EnumInfo, ExtensionInfo, ForeignKeyInfo, FunctionInfo,
    IndexInfo, PolicyInfo, RoleInfo, SequenceInfo, TableInfo, TriggerInfo, ViewInfo,
};
use std::collections::HashMap;

pub mod objects;
pub mod summary;
pub mod tables;
pub mod utils;

#[derive(Debug)]
pub struct SchemaDiff {
    pub tables_to_create: Vec<String>,
    pub tables_to_drop: Vec<String>,
    pub table_changes: HashMap<String, TableDiff>,
    pub enum_changes: Vec<EnumChange>,
    pub functions_to_create: Vec<FunctionInfo>,
    pub functions_to_drop: Vec<String>,
    pub functions_to_update: Vec<FunctionInfo>,
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
    pub roles_to_create: Vec<RoleInfo>,
    pub roles_to_drop: Vec<String>,
    pub roles_to_update: Vec<RoleInfo>,
}

#[derive(Debug)]
pub struct TableDiff {
    pub columns_to_add: Vec<String>,
    pub columns_to_drop: Vec<String>,
    pub columns_to_modify: Vec<ColumnModification>,
    pub rls_change: Option<bool>,
    pub comment_change: Option<Option<String>>,
    pub policies_to_create: Vec<PolicyInfo>,
    pub policies_to_drop: Vec<PolicyInfo>,
    pub triggers_to_create: Vec<TriggerInfo>,
    pub triggers_to_drop: Vec<TriggerInfo>,
    pub indexes_to_create: Vec<IndexInfo>,
    pub indexes_to_drop: Vec<IndexInfo>,
    pub check_constraints_to_create: Vec<crate::schema::CheckConstraintInfo>,
    pub check_constraints_to_drop: Vec<crate::schema::CheckConstraintInfo>,
    pub foreign_keys_to_create: Vec<ForeignKeyInfo>,
    pub foreign_keys_to_drop: Vec<ForeignKeyInfo>,
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
    pub identity_change: Option<(Option<String>, Option<String>)>,
    pub collation_change: Option<(Option<String>, Option<String>)>,
    pub comment_change: Option<(Option<String>, Option<String>)>,
}

#[derive(Debug)]
pub struct EnumChange {
    pub name: String,
    pub type_: EnumChangeType,
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
        roles_to_create: vec![],
        roles_to_drop: vec![],
        roles_to_update: vec![],
    };

    // Tables
    for (name, local_table) in &local.tables {
        if !remote.tables.contains_key(name) {
            diff.tables_to_create.push(name.clone());
        } else {
            let remote_table = remote.tables.get(name).unwrap();
            let table_diff = tables::compute_table_diff(remote_table, local_table);
            if !table_diff.is_empty() {
                diff.table_changes.insert(name.clone(), table_diff);
            }
        }
    }

    for (name, _) in &remote.tables {
        if !local.tables.contains_key(name) {
            diff.tables_to_drop.push(name.clone());
        }
    }

    // Enums
    for (name, local_enum) in &local.enums {
        if !remote.enums.contains_key(name) {
            diff.enum_changes.push(EnumChange {
                name: name.clone(),
                type_: EnumChangeType::Create,
                values_to_add: Some(local_enum.values.clone()), // Include all values for new enum
            });
        } else {
            let remote_enum = remote.enums.get(name).unwrap();
            if local_enum.values != remote_enum.values {
                let mut values_to_add = vec![];
                for val in &local_enum.values {
                    if !remote_enum.values.contains(val) {
                        values_to_add.push(val.clone());
                    }
                }
                if !values_to_add.is_empty() {
                    diff.enum_changes.push(EnumChange {
                        name: name.clone(),
                        type_: EnumChangeType::AddValue,
                        values_to_add: Some(values_to_add),
                    });
                }
            }
        }
    }

    for (name, _) in &remote.enums {
        if !local.enums.contains_key(name) {
            diff.enum_changes.push(EnumChange {
                name: name.clone(),
                type_: EnumChangeType::Drop,
                values_to_add: None,
            });
        }
    }

    // Extensions
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

    // Functions
    for (name, local_func) in &local.functions {
        if !remote.functions.contains_key(name) {
            diff.functions_to_create.push(local_func.clone());
        } else {
            let remote_func = remote.functions.get(name).unwrap();
            if local_func.definition != remote_func.definition
                || local_func.return_type != remote_func.return_type
                || local_func.language != remote_func.language
            {
                diff.functions_to_update.push(local_func.clone());
            }
        }
    }
    for (name, _) in &remote.functions {
        if !local.functions.contains_key(name) {
            diff.functions_to_drop.push(name.clone());
        }
    }

    // Views
    for (name, local_view) in &local.views {
        if !remote.views.contains_key(name) {
            diff.views_to_create.push(local_view.clone());
        } else {
            let remote_view = remote.views.get(name).unwrap();
            if objects::views_differ(local_view, remote_view) {
                diff.views_to_update.push(local_view.clone());
            }
        }
    }
    for (name, _) in &remote.views {
        if !local.views.contains_key(name) {
            diff.views_to_drop.push(name.clone());
        }
    }

    // Sequences
    for (name, local_seq) in &local.sequences {
        if !remote.sequences.contains_key(name) {
            diff.sequences_to_create.push(local_seq.clone());
        } else {
            let remote_seq = remote.sequences.get(name).unwrap();
            if objects::sequences_differ(local_seq, remote_seq) {
                diff.sequences_to_update.push(local_seq.clone());
            }
        }
    }
    for (name, _) in &remote.sequences {
        if !local.sequences.contains_key(name) {
            diff.sequences_to_drop.push(name.clone());
        }
    }

    // Composite Types
    for (name, local_type) in &local.composite_types {
        if !remote.composite_types.contains_key(name) {
            diff.composite_types_to_create.push(local_type.clone());
        }
    }
    for (name, _) in &remote.composite_types {
        if !local.composite_types.contains_key(name) {
            diff.composite_types_to_drop.push(name.clone());
        }
    }

    // Domains
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

    // Roles
    for (name, local_role) in &local.roles {
        if !remote.roles.contains_key(name) {
            diff.roles_to_create.push(local_role.clone());
        } else {
            let remote_role = remote.roles.get(name).unwrap();
            if local_role != remote_role {
                diff.roles_to_update.push(local_role.clone());
            }
        }
    }
    for (name, _) in &remote.roles {
        if !local.roles.contains_key(name) {
            diff.roles_to_drop.push(name.clone());
        }
    }

    diff
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

    pub fn is_destructive(&self) -> bool {
        // Dropping columns is destructive
        if !self.columns_to_drop.is_empty() {
            return true;
        }

        // Modifying columns can be destructive (e.g. type changes)
        // We consider any modification destructive for safety, or check specific types
        for modification in &self.columns_to_modify {
            // Type change is potentially destructive
            if modification.changes.type_change.is_some() {
                return true;
            }
            // Nullable change (True -> False) might fail but isn't strictly "data loss" 
            // unless data is lost. We often treat it as "dangerous" though.
            // keeping it simple: type change is the big one.
        }

        // Dropping policies/triggers/indexes/constraints is NOT considered destructive 
        // in the sense of data loss, though it changes behavior.
        // User asked for "something destructive", usually implies data.

        false
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
            && self.roles_to_create.is_empty()
            && self.roles_to_drop.is_empty()
            && self.roles_to_update.is_empty()
    }

    pub fn is_destructive(&self) -> bool {
        if !self.tables_to_drop.is_empty() {
            return true;
        }

        for table_diff in self.table_changes.values() {
            if table_diff.is_destructive() {
                return true;
            }
        }

        for enum_change in &self.enum_changes {
            if enum_change.type_ == EnumChangeType::Drop {
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests;
