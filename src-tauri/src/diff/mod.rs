use crate::defaults;
use crate::schema::{
    CompositeTypeInfo, DbSchema, DomainInfo, EnumInfo, ExtensionInfo, ForeignKeyInfo, FunctionGrant, FunctionInfo,
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
    pub generated_change: Option<(Option<String>, Option<String>)>,
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

    for (name, table) in &remote.tables {
        // If table belongs to an extension, don't drop it (let DROP EXTENSION handle it, or ignore it)
        if table.extension.is_some() {
            continue;
        }
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

    for (name, enum_) in &remote.enums {
        // If enum belongs to an extension, don't drop it
        if enum_.extension.is_some() {
            continue;
        }
        if !local.enums.contains_key(name) {
            diff.enum_changes.push(EnumChange {
                name: name.clone(),
                type_: EnumChangeType::Drop,
                values_to_add: None,
            });
        }
    }

    // Extensions (filter out default Supabase extensions)
    for (name, local_ext) in &local.extensions {
        if defaults::is_default_extension(name) {
            continue; // Skip default extensions
        }
        if !remote.extensions.contains_key(name) {
            diff.extensions_to_create.push(local_ext.clone());
        }
    }
    for (name, _) in &remote.extensions {
        if defaults::is_default_extension(name) {
            continue; // Skip default extensions
        }
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
            
            // Check if argument names have changed (Postgres doesn't support renaming args with CREATE OR REPLACE)
            // The key is based on types, so if we are here, types match. We check names.
            let args_renamed = local_func.args.len() == remote_func.args.len() && 
                local_func.args.iter().zip(&remote_func.args).any(|(l, r)| l.name != r.name);

            if args_renamed {
                eprintln!("=== FUNCTION DIFF DEBUG for {} ===", name);
                eprintln!("  Argument names changed - forcing DROP and CREATE");
                diff.functions_to_drop.push(name.clone());
                diff.functions_to_create.push(local_func.clone());
                continue;
            }

            // Normalize function definitions before comparison to handle formatting differences
            // (dollar quoting, quoted identifiers, whitespace)
            let local_def_normalized = utils::normalize_function_definition(&local_func.definition);
            let remote_def_normalized = utils::normalize_function_definition(&remote_func.definition);
            let local_return_normalized = local_func.return_type.to_lowercase();
            let remote_return_normalized = remote_func.return_type.to_lowercase();
            
            let def_changed = local_def_normalized != remote_def_normalized;
            let return_changed = local_return_normalized != remote_return_normalized;
            let lang_changed = local_func.language.to_lowercase() != remote_func.language.to_lowercase();
            let security_definer_changed = local_func.security_definer != remote_func.security_definer;
            let config_params_changed = !config_params_match(&local_func.config_params, &remote_func.config_params);
            // Only compare grants if local schema explicitly defines grants
            // (skip if local has no grants, since users likely haven't added GRANT statements to their schema files)
            let grants_changed = !local_func.grants.is_empty() && !grants_match(&local_func.grants, &remote_func.grants);
            
            if def_changed || return_changed || lang_changed || security_definer_changed || config_params_changed || grants_changed {
                eprintln!("=== FUNCTION DIFF DEBUG for {} ===", name);
                if def_changed {
                    eprintln!("  Definition changed:");
                    eprintln!("    Local:  {}", local_def_normalized.chars().take(100).collect::<String>());
                    eprintln!("    Remote: {}", remote_def_normalized.chars().take(100).collect::<String>());
                }
                if return_changed {
                    eprintln!("  Return type changed: '{}' vs '{}'", local_return_normalized, remote_return_normalized);
                }
                if lang_changed {
                    eprintln!("  Language changed: '{}' vs '{}'", local_func.language, remote_func.language);
                }
                if security_definer_changed {
                    eprintln!("  Security definer changed: {} vs {}", local_func.security_definer, remote_func.security_definer);
                }
                if config_params_changed {
                    eprintln!("  Config params changed: {:?} vs {:?}", local_func.config_params, remote_func.config_params);
                }
                if grants_changed {
                    eprintln!("  Grants changed: {:?} vs {:?}", local_func.grants, remote_func.grants);
                }
                diff.functions_to_update.push(local_func.clone());
            }
        }
    }
    for (name, func) in &remote.functions {
        // If function belongs to an extension, don't drop it
        if func.extension.is_some() {
            continue;
        }
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
    for (name, view) in &remote.views {
        // If view belongs to an extension, don't drop it
        if view.extension.is_some() {
            continue;
        }
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
    for (name, seq) in &remote.sequences {
        // If sequence belongs to an extension, don't drop it
        if seq.extension.is_some() {
            continue;
        }
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
    for (name, type_) in &remote.composite_types {
        // If composite type belongs to an extension, don't drop it
        if type_.extension.is_some() {
            continue;
        }
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
    for (name, domain) in &remote.domains {
        // If domain belongs to an extension, don't drop it
        if domain.extension.is_some() {
            continue;
        }
        if !local.domains.contains_key(name) {
            diff.domains_to_drop.push(name.clone());
        }
    }

    // Roles (filter out default Supabase roles)
    for (name, local_role) in &local.roles {
        if defaults::is_default_role(name) {
            continue; // Skip default roles
        }
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
        if defaults::is_default_role(name) {
            continue; // Skip default roles
        }
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

/// Compare two lists of function grants for equality, ignoring order
fn grants_match(local: &[FunctionGrant], remote: &[FunctionGrant]) -> bool {
    // Filter implicit system grants from remote before comparison
    let remote_filtered: Vec<&FunctionGrant> = remote.iter().filter(|r| {
        let name = r.grantee.as_str();
        
        // Always ignore postgres (owner) and system roles
        if name == "postgres" || name == "supabase_admin" {
            return false;
        }

        // For default roles, only include them if they are present in local definition
        // This implies: if local doesn't mention them, we ignore whatever the remote has (implicit default)
        if name == "anon" || name == "service_role" || name == "public" || name == "authenticated" {
            return local.iter().any(|l| l.grantee == name);
        }

        true
    }).collect();

    // If local has no grants, and we filtered everything from remote (or remote only had defaults we filtered out), match.
    if local.is_empty() && remote_filtered.is_empty() {
        return true;
    }

    // But if local has valid grants, check counts against the filtered remote list
    if local.len() != remote_filtered.len() {
        return false;
    }
    
    // Check that every local grant exists in remote
    for grant in local {
        if !remote.iter().any(|r| r.grantee == grant.grantee && r.privilege == grant.privilege) {
            return false;
        }
    }
    
    true
}

/// Normalize a config param value by stripping surrounding quotes
fn normalize_config_value(value: &str) -> String {
    value.trim_matches('"').trim_matches('\'').to_string()
}

/// Compare config params, normalizing values before comparison
fn config_params_match(local: &[(String, String)], remote: &[(String, String)]) -> bool {
    if local.len() != remote.len() {
        return false;
    }
    
    for (local_key, local_val) in local {
        let local_normalized = normalize_config_value(local_val);
        if !remote.iter().any(|(rkey, rval)| {
            rkey == local_key && normalize_config_value(rval) == local_normalized
        }) {
            return false;
        }
    }
    
    true
}

#[cfg(test)]
mod tests;




