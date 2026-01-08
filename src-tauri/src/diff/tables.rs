use super::utils::{normalize_option, normalize_sql};
use crate::diff::{ColumnChangeDetail, ColumnModification, TableDiff};
use crate::schema::{
    CheckConstraintInfo, ForeignKeyInfo, IndexInfo, PolicyInfo, TableInfo, TriggerInfo,
};
use std::collections::HashMap;

pub fn compute_table_diff(remote: &TableInfo, local: &TableInfo) -> TableDiff {
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
                identity_change: None,
                collation_change: None,
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

            // Default value - normalize for case-insensitive comparison
            if normalize_option(&local_col.column_default) != normalize_option(&remote_col.column_default) {
                changes.default_change = Some((
                    remote_col.column_default.clone(),
                    local_col.column_default.clone(),
                ));
            }

            // Identity Generation
            if local_col.identity_generation != remote_col.identity_generation {
                changes.identity_change = Some((
                    remote_col.identity_generation.clone(),
                    local_col.identity_generation.clone(),
                ));
            }

            // Collation
            if local_col.collation != remote_col.collation {
                changes.collation_change = Some((
                    remote_col.collation.clone(),
                    local_col.collation.clone(),
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
                || changes.identity_change.is_some()
                || changes.collation_change.is_some()
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
            if policies_differ(p, remote_p) {
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

pub fn policies_differ(local: &PolicyInfo, remote: &PolicyInfo) -> bool {
    // Command must match
    if local.cmd.to_uppercase() != remote.cmd.to_uppercase() {
        return true;
    }
    
    // Normalize and compare roles (sort for consistent comparison)
    let mut local_roles: Vec<String> = local.roles.iter().map(|r| r.to_lowercase()).collect();
    let mut remote_roles: Vec<String> = remote.roles.iter().map(|r| r.to_lowercase()).collect();
    local_roles.sort();
    remote_roles.sort();
    if local_roles != remote_roles {
        return true;
    }
    
    // Normalize and compare expressions
    if normalize_option(&local.qual) != normalize_option(&remote.qual) {
        return true;
    }
    
    if normalize_option(&local.with_check) != normalize_option(&remote.with_check) {
        return true;
    }
    
    false
}

pub fn triggers_differ(local: &TriggerInfo, remote: &TriggerInfo) -> bool {
    local.events != remote.events
        || local.timing != remote.timing
        || local.orientation != remote.orientation
        || local.function_name != remote.function_name
        || local.when_clause != remote.when_clause
}

pub fn indexes_differ(local: &IndexInfo, remote: &IndexInfo) -> bool {
    local.columns != remote.columns
        || local.is_unique != remote.is_unique
        || local.is_primary != remote.is_primary
        || local.index_method.to_lowercase() != remote.index_method.to_lowercase()
        || normalize_option(&local.where_clause) != normalize_option(&remote.where_clause)
        || local.expressions != remote.expressions
}

pub fn foreign_keys_differ(local: &ForeignKeyInfo, remote: &ForeignKeyInfo) -> bool {
    local.column_name != remote.column_name
        || local.foreign_table != remote.foreign_table
        || local.foreign_column != remote.foreign_column
        || local.on_delete != remote.on_delete
        || local.on_update != remote.on_update
}
