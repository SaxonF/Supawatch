use super::utils;
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
                generated_change: None,
                comment_change: None,
            };

            // Generated Columns
            // We use generation_expression as the source of truth.
            // Helper to get normalized expression for comparison
            fn normalize_gen_expr(expr: &Option<String>) -> Option<String> {
                expr.as_ref().map(|s| {
                    let mut trimmed = s.trim();
                    // Remove outer parens if present
                    while trimmed.starts_with('(') && trimmed.ends_with(')') {
                         trimmed = trimmed[1..trimmed.len()-1].trim();
                    }

                    // 1. Case insensitivity (ignoring quoted strings)
                    // Do this FIRST so that everything else (stripping casts, public, etc) works on lowercase
                    let mut lowercased = String::with_capacity(trimmed.len());
                    let mut in_quote = false;
                    let mut chars_iter = trimmed.chars().peekable();
                    
                    while let Some(c) = chars_iter.next() {
                        if c == '\'' {
                            // Check for escaped quote (e.g. 'O''Neil')
                            if in_quote {
                                if let Some(&next_c) = chars_iter.peek() {
                                    if next_c == '\'' {
                                        lowercased.push(c);
                                        lowercased.push(chars_iter.next().unwrap());
                                        continue;
                                    }
                                }
                            }
                            in_quote = !in_quote;
                            lowercased.push(c);
                        } else {
                            if in_quote {
                                lowercased.push(c);
                            } else {
                                lowercased.push(c.to_ascii_lowercase());
                            }
                        }
                    }
                    
                    // 2. Remove "public." prefix
                    let without_public = lowercased.replace("public.", "");
                    
                    // 3. Collapse whitespace
                    let mut collapsed = without_public.split_whitespace().collect::<Vec<_>>().join(" ");
                    
                    // 4. Strip common type casts
                    let type_cast_suffixes = [
                        "::text", "::regconfig", "::integer", "::int", "::bigint", "::smallint",
                        "::boolean", "::bool", "::numeric", "::jsonb", "::varchar",
                        "::character varying", "::timestamp", "::timestamptz", "::date",
                        "::time", "::float", "::double precision", "::regclass", "::regtype",
                        "::bpchar",
                    ];
                    
                    for suffix in type_cast_suffixes {
                        collapsed = collapsed.replace(suffix, "");
                    }

                    // 5. Remove inner parentheses found in concatenations or expressions
                    let mut s = collapsed;
                    let mut changed = true;
                    while changed {
                        changed = false;
                        let mut new_s = String::with_capacity(s.len());
                        let chars: Vec<char> = s.chars().collect();
                        let mut i = 0;
                        while i < chars.len() {
                            if chars[i] == '(' {
                                // check ahead for matching ) without commas or other parens
                                let mut j = i + 1;
                                let mut has_comma = false;
                                let mut has_paren = false;
                                let mut found = false;
                                while j < chars.len() {
                                    if chars[j] == '(' { has_paren = true; break; }
                                    if chars[j] == ',' { has_comma = true; } 
                                    if chars[j] == ')' { found = true; break; }
                                    j += 1;
                                }
                                
                                if found && !has_paren && !has_comma {
                                    // It's a simple group (a) or ("a") or ('a')
                                    // Check if it's a function call?
                                    // Only if `i > 0` and chars[i-1] is identifier char.
                                    let is_func = if i > 0 {
                                        let prev = chars[i-1];
                                        prev.is_alphanumeric() || prev == '_'
                                    } else {
                                        false
                                    };
                                    
                                    if !is_func {
                                        // Remove parens
                                        // push content from i+1 to j
                                        for k in i+1..j {
                                            new_s.push(chars[k]);
                                        }
                                        i = j + 1;
                                        changed = true;
                                        continue;
                                    }
                                }
                            }
                            new_s.push(chars[i]);
                            i += 1;
                        }
                        if changed {
                            s = new_s;
                        }
                    }
                    
                    s
                })
            }

            let normalized_local = normalize_gen_expr(&local_col.generation_expression);
            let normalized_remote = normalize_gen_expr(&remote_col.generation_expression);

            let generated_changed = local_col.is_generated != remote_col.is_generated || 
               normalized_local != normalized_remote;

            if generated_changed {
                println!("[DIFF] Generated column '{}' changed:", name);
                println!("[DIFF]   Local raw:       {:?}", local_col.generation_expression);
                println!("[DIFF]   Remote raw:      {:?}", remote_col.generation_expression);
                println!("[DIFF]   Local norm:      {:?}", normalized_local);
                println!("[DIFF]   Remote norm:     {:?}", normalized_remote);
                
                // Generated column changes require DROP and ADD
                diff.columns_to_drop.push(name.clone());
                diff.columns_to_add.push(name.clone());
                continue;
            }

            // Type comparison (normalized)
            if utils::normalize_data_type(&local_col.data_type) != utils::normalize_data_type(&remote_col.data_type) {
                changes.type_change =
                    Some((remote_col.data_type.clone(), local_col.data_type.clone()));
            }

            // Nullability
            if local_col.is_nullable != remote_col.is_nullable {
                changes.nullable_change = Some((remote_col.is_nullable, local_col.is_nullable));
            }

            // Default value - normalize for comparison (strips type casts like ::text)
            // Skip comparison for generated columns - they can't have defaults
            if !local_col.is_generated && !remote_col.is_generated {
                if utils::normalize_default_option(&local_col.column_default) != utils::normalize_default_option(&remote_col.column_default) {
                    changes.default_change = Some((
                        remote_col.column_default.clone(),
                        local_col.column_default.clone(),
                    ));
                }
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
                || changes.generated_change.is_some()
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
        }
        // Note: We don't compare expressions because PostgreSQL rewrites them internally
        // (e.g., IN ('a', 'b') becomes = ANY (ARRAY['a', 'b']))
        // If constraint names match, we consider them equivalent
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
        eprintln!("=== POLICY DIFF DEBUG for {} ===", local.name);
        eprintln!("CMD DIFFERS: local={} remote={}", local.cmd, remote.cmd);
        eprintln!("=== END DEBUG ===");
        return true;
    }
    
    // Normalize and compare roles (sort for consistent comparison)
    let mut local_roles: Vec<String> = local.roles.iter().map(|r| r.to_lowercase()).collect();
    let mut remote_roles: Vec<String> = remote.roles.iter().map(|r| r.to_lowercase()).collect();
    local_roles.sort();
    remote_roles.sort();
    if local_roles != remote_roles {
        eprintln!("=== POLICY DIFF DEBUG for {} ===", local.name);
        eprintln!("ROLES DIFFER: local={:?} remote={:?}", local_roles, remote_roles);
        eprintln!("=== END DEBUG ===");
        return true;
    }
    
    // Normalize and compare expressions
    let local_qual_normalized = utils::normalize_option(&local.qual);
    let remote_qual_normalized = utils::normalize_option(&remote.qual);
    if local_qual_normalized != remote_qual_normalized {
        eprintln!("=== POLICY DIFF DEBUG for {} ===", local.name);
        eprintln!("QUAL DIFFERS:");
        eprintln!("  LOCAL raw: {:?}", local.qual);
        eprintln!("  REMOTE raw: {:?}", remote.qual);
        eprintln!("  LOCAL normalized: {:?}", local_qual_normalized);
        eprintln!("  REMOTE normalized: {:?}", remote_qual_normalized);
        eprintln!("=== END DEBUG ===");
        return true;
    }
    
    let local_with_check_normalized = utils::normalize_option(&local.with_check);
    let remote_with_check_normalized = utils::normalize_option(&remote.with_check);
    if local_with_check_normalized != remote_with_check_normalized {
        eprintln!("=== POLICY DIFF DEBUG for {} ===", local.name);
        eprintln!("WITH_CHECK DIFFERS:");
        eprintln!("  LOCAL raw: {:?}", local.with_check);
        eprintln!("  REMOTE raw: {:?}", remote.with_check);
        eprintln!("  LOCAL normalized: {:?}", local_with_check_normalized);
        eprintln!("  REMOTE normalized: {:?}", remote_with_check_normalized);
        eprintln!("=== END DEBUG ===");
        return true;
    }
    
    false
}

pub fn triggers_differ(local: &TriggerInfo, remote: &TriggerInfo) -> bool {
    let mut differs = false;

    if local.events != remote.events {
        eprintln!("=== TRIGGER DIFF DEBUG for {} ===", local.name);
        eprintln!(
            "EVENTS DIFFER: local={:?} remote={:?}",
            local.events, remote.events
        );
        eprintln!("=== END DEBUG ===");
        differs = true;
    }

    if local.timing != remote.timing {
        eprintln!("=== TRIGGER DIFF DEBUG for {} ===", local.name);
        eprintln!(
            "TIMING DIFFERS: local={} remote={}",
            local.timing, remote.timing
        );
        eprintln!("=== END DEBUG ===");
        differs = true;
    }

    if local.orientation != remote.orientation {
        eprintln!("=== TRIGGER DIFF DEBUG for {} ===", local.name);
        eprintln!(
            "ORIENTATION DIFFERS: local={} remote={}",
            local.orientation, remote.orientation
        );
        eprintln!("=== END DEBUG ===");
        differs = true;
    }

    // Normalize both function names: if they don't have a schema, assume "public"
    let local_func_normalized = if local.function_name.contains('.') {
        std::borrow::Cow::Borrowed(local.function_name.as_str())
    } else {
        std::borrow::Cow::Owned(format!("public.{}", local.function_name))
    };

    let remote_func_normalized = if remote.function_name.contains('.') {
        std::borrow::Cow::Borrowed(remote.function_name.as_str())
    } else {
        std::borrow::Cow::Owned(format!("public.{}", remote.function_name))
    };

    if local_func_normalized != remote_func_normalized {
        eprintln!("=== TRIGGER DIFF DEBUG for {} ===", local.name);
        eprintln!(
            "FUNCTION NAME DIFFERS: local={} remote={}",
            local.function_name, remote.function_name
        );
        eprintln!(
            "NORMALIZED: local={} remote={}",
            local_func_normalized, remote_func_normalized
        );
        eprintln!("=== END DEBUG ===");
        differs = true;
    }

    let local_when = utils::normalize_option(&local.when_clause);
    let remote_when = utils::normalize_option(&remote.when_clause);

    if local_when != remote_when {
        eprintln!("=== TRIGGER DIFF DEBUG for {} ===", local.name);
        eprintln!("WHEN CLAUSE DIFFERS:");
        eprintln!("  LOCAL raw: {:?}", local.when_clause);
        eprintln!("  REMOTE raw: {:?}", remote.when_clause);
        eprintln!("  LOCAL normalized: {:?}", local_when);
        eprintln!("  REMOTE normalized: {:?}", remote_when);
        eprintln!("=== END DEBUG ===");
        differs = true;
    }

    differs
}

pub fn indexes_differ(local: &IndexInfo, remote: &IndexInfo) -> bool {
    if local.columns != remote.columns {
        println!("[DIFF] Index '{}' COLUMNS differ: local={:?} remote={:?}", local.index_name, local.columns, remote.columns);
        return true;
    }
    if local.is_unique != remote.is_unique {
        println!("[DIFF] Index '{}' IS_UNIQUE differs: local={} remote={}", local.index_name, local.is_unique, remote.is_unique);
        return true;
    }
    if local.is_primary != remote.is_primary {
        println!("[DIFF] Index '{}' IS_PRIMARY differs: local={} remote={}", local.index_name, local.is_primary, remote.is_primary);
        return true;
    }
    if local.index_method.to_lowercase() != remote.index_method.to_lowercase() {
        println!("[DIFF] Index '{}' METHOD differs: local={} remote={}", local.index_name, local.index_method, remote.index_method);
        return true;
    }
    let local_where_normalized = utils::normalize_option(&local.where_clause);
    let remote_where_normalized = utils::normalize_option(&remote.where_clause);
    if local_where_normalized != remote_where_normalized {
        println!("[DIFF] Index '{}' WHERE differs:", local.index_name);
        println!("[DIFF]   local raw:  {:?}", local.where_clause);
        println!("[DIFF]   remote raw: {:?}", remote.where_clause);
        println!("[DIFF]   local norm: {:?}", local_where_normalized);
        println!("[DIFF]   remote norm: {:?}", remote_where_normalized);
        return true;
    }
    // Normalize expressions before comparing: lowercase, strip quotes, collapse whitespace, strip type casts
    let normalize_expr = |e: &str| -> String {
        let s = e.to_lowercase().replace('"', "");
        let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
        // Strip common type casts (e.g., ::uuid, ::text, ::integer)
        let type_cast_suffixes = [
            "::uuid", "::text", "::integer", "::int", "::bigint", "::smallint",
            "::boolean", "::bool", "::numeric", "::jsonb", "::varchar",
            "::character varying", "::timestamp", "::timestamptz", "::date",
            "::time", "::float", "::double precision", "::regclass", "::regtype",
        ];
        let mut result = collapsed;
        for suffix in type_cast_suffixes {
            result = result.replace(suffix, "");
        }
        result
    };
    let local_exprs: Vec<String> = local.expressions.iter().map(|e| normalize_expr(e)).collect();
    let remote_exprs: Vec<String> = remote.expressions.iter().map(|e| normalize_expr(e)).collect();
    if local_exprs != remote_exprs {
        println!("[DIFF] Index '{}' EXPRESSIONS differ:", local.index_name);
        println!("[DIFF]   local raw:  {:?}", local.expressions);
        println!("[DIFF]   remote raw: {:?}", remote.expressions);
        println!("[DIFF]   local norm: {:?}", local_exprs);
        println!("[DIFF]   remote norm: {:?}", remote_exprs);
        return true;
    }
    false
}

pub fn foreign_keys_differ(local: &ForeignKeyInfo, remote: &ForeignKeyInfo) -> bool {
    local.column_name != remote.column_name
        || local.foreign_schema != remote.foreign_schema
        || local.foreign_table != remote.foreign_table
        || local.foreign_column != remote.foreign_column
        || local.on_delete != remote.on_delete
        || local.on_update != remote.on_update
}
