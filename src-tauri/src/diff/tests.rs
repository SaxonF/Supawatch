use super::*;
use crate::schema::*;
use std::collections::HashMap;

#[test]
fn test_create_table() {
    let mut local = DbSchema::new();
    let table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    local.tables.insert("users".into(), table);

    let remote = DbSchema::new();

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.tables_to_create, vec!["users"]);
    assert!(diff.tables_to_drop.is_empty());
}

#[test]
fn test_drop_table() {
    let mut remote = DbSchema::new();
    let table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    remote.tables.insert("users".into(), table);

    let local = DbSchema::new();

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.tables_to_drop, vec!["users"]);
    assert!(diff.tables_to_create.is_empty());
}

#[test]
fn test_add_column() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    
    let mut local_table = remote_table.clone();
    
    local_table.columns.insert("email".into(), ColumnInfo {
        column_name: "email".into(),
        data_type: "text".into(),
        is_nullable: false,
        column_default: None,
        udt_name: "text".into(),
        is_primary_key: false,
        is_unique: true,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    remote.tables.insert("users".into(), remote_table);
    local.tables.insert("users".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("users").unwrap();
    assert_eq!(table_diff.columns_to_add, vec!["email"]);
}

#[test]
fn test_drop_column() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    
    remote_table.columns.insert("email".into(), ColumnInfo {
        column_name: "email".into(),
        data_type: "text".into(),
        is_nullable: false,
        column_default: None,
        udt_name: "text".into(),
        is_primary_key: false,
        is_unique: true,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    let local_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    remote.tables.insert("users".into(), remote_table);
    local.tables.insert("users".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("users").unwrap();
    assert_eq!(table_diff.columns_to_drop, vec!["email"]);
}

#[test]
fn test_modify_column_type() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    
    remote_table.columns.insert("age".into(), ColumnInfo {
        column_name: "age".into(),
        data_type: "integer".into(),
        is_nullable: true,
        column_default: None,
        udt_name: "int4".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });
    
    let mut local_table = remote_table.clone();
    local_table.columns.insert("age".into(), ColumnInfo {
        column_name: "age".into(),
        data_type: "bigint".into(), // Changed type
        is_nullable: true,
        column_default: None,
        udt_name: "int8".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    remote.tables.insert("users".into(), remote_table);
    local.tables.insert("users".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("users").unwrap();
    assert_eq!(table_diff.columns_to_modify.len(), 1);
    let change = &table_diff.columns_to_modify[0];
    assert_eq!(change.column_name, "age");
    assert_eq!(change.changes.type_change, Some(("integer".into(), "bigint".into())));
}

#[test]
fn test_modify_column_nullable() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    remote_table.columns.insert("email".into(), ColumnInfo {
        column_name: "email".into(),
        data_type: "text".into(),
        is_nullable: true, // Initially nullable
        column_default: None,
        udt_name: "text".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    let mut local_table = remote_table.clone();
    local_table.columns.insert("email".into(), ColumnInfo {
        column_name: "email".into(),
        data_type: "text".into(),
        is_nullable: false, // Now NOT NULL
        column_default: None,
        udt_name: "text".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    remote.tables.insert("users".into(), remote_table);
    local.tables.insert("users".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("users").unwrap();
    assert_eq!(table_diff.columns_to_modify.len(), 1);
    let change = &table_diff.columns_to_modify[0];
    assert_eq!(change.changes.nullable_change, Some((true, false)));
}

#[test]
fn test_modify_generated_column_expression() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "products".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    
    remote_table.columns.insert("total".into(), ColumnInfo {
        column_name: "total".into(),
        data_type: "numeric".into(),
        is_nullable: true,
        column_default: None,
        udt_name: "numeric".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,

        is_generated: true,
        generation_expression: Some("(price * qty)".into()),
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });
    
    let mut local_table = remote_table.clone();
    local_table.columns.insert("total".into(), ColumnInfo {
        column_name: "total".into(),
        data_type: "numeric".into(),
        is_nullable: true,
        column_default: None,
        udt_name: "numeric".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,

        is_generated: true,
        generation_expression: Some("(price + qty)".into()), // Changed expression
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    remote.tables.insert("products".into(), remote_table);
    local.tables.insert("products".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("products").unwrap();
    
    // Generated column changes are handled as DROP + ADD
    assert!(table_diff.columns_to_drop.contains(&"total".to_string()));
    assert!(table_diff.columns_to_add.contains(&"total".to_string()));
}

#[test]
fn test_add_check_constraint() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let mut local_table = remote_table.clone();
    local_table.check_constraints.push(CheckConstraintInfo {
        name: "age_positive".into(),
        expression: "age > 0".into(),
        columns: vec!["age".into()],
    });

    remote.tables.insert("users".into(), remote_table);
    local.tables.insert("users".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("users").unwrap();
    assert_eq!(table_diff.check_constraints_to_create.len(), 1);
    assert_eq!(table_diff.check_constraints_to_create[0].name, "age_positive");
}

#[test]
fn test_drop_check_constraint() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    remote_table.check_constraints.push(CheckConstraintInfo {
        name: "age_positive".into(),
        expression: "age > 0".into(),
        columns: vec!["age".into()],
    });

    let local_table = remote_table.clone();
    let mut local_table = local_table;
    local_table.check_constraints.clear();

    remote.tables.insert("users".into(), remote_table);
    local.tables.insert("users".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("users").unwrap();
    assert_eq!(table_diff.check_constraints_to_drop.len(), 1);
    assert_eq!(table_diff.check_constraints_to_drop[0].name, "age_positive");
}

#[test]
fn test_create_enum() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    local.enums.insert("status".into(), EnumInfo {
        schema: "public".into(),
        name: "status".into(),
        values: vec!["active".into(), "inactive".into()], extension: None,
    });

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.enum_changes.len(), 1);
    assert_eq!(diff.enum_changes[0].type_, EnumChangeType::Create);
}

#[test]
fn test_drop_enum() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    remote.enums.insert("status".into(), EnumInfo {
        schema: "public".into(),
        name: "status".into(),
        values: vec!["active".into(), "inactive".into()], extension: None,
    });

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.enum_changes.len(), 1);
    assert_eq!(diff.enum_changes[0].type_, EnumChangeType::Drop);
}

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
        roles_to_create: vec![],
        roles_to_drop: vec![],
        roles_to_update: vec![],
    };

    let summary = diff.summarize();
    assert!(summary.contains("+ Table 'users'"));
    assert!(summary.contains("- Table 'posts'"));
}

#[test]
fn test_enum_add_value() {
    let mut remote = DbSchema::new();
    remote.enums.insert(
        "status".to_string(),
        EnumInfo {
            schema: "public".to_string(),
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()], extension: None,
        },
    );

    let mut local = DbSchema::new();
    local.enums.insert(
        "status".to_string(),
        EnumInfo {
            schema: "public".to_string(),
            name: "status".to_string(),
            values: vec![
                "active".to_string(),
                "inactive".to_string(),
                "pending".to_string(),
            ],
            extension: None,
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

    assert!(tables::indexes_differ(&local, &remote));
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

    assert!(tables::triggers_differ(&local, &remote));
}

#[test]
fn test_foreign_key_on_update_comparison() {
    let local = ForeignKeyInfo {
        constraint_name: "fk_test".to_string(),
        columns: vec!["user_id".to_string()],
        foreign_schema: "public".to_string(),
        foreign_table: "users".to_string(),
        foreign_columns: vec!["id".to_string()],
        on_delete: "CASCADE".to_string(),
        on_update: "SET NULL".to_string(),
    };

    let remote = ForeignKeyInfo {
        constraint_name: "fk_test".to_string(),
        columns: vec!["user_id".to_string()],
        foreign_schema: "public".to_string(),
        foreign_table: "users".to_string(),
        foreign_columns: vec!["id".to_string()],
        on_delete: "CASCADE".to_string(),
        on_update: "NO ACTION".to_string(),
    };

    assert!(tables::foreign_keys_differ(&local, &remote));
}

#[test]
fn test_destructive_change_detection() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    // 1. Drop Table -> Destructive
    remote.tables.insert("users".into(), TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    });
    // Local empty -> Drop table
    let diff = compute_diff(&remote, &local);
    assert!(diff.is_destructive(), "Dropping a table should be destructive");

    // 2. Drop Column -> Destructive
    let mut remote_with_col = remote.clone();
    remote_with_col.tables.get_mut("users").unwrap().columns.insert("email".into(), ColumnInfo {
        column_name: "email".into(),
        data_type: "text".into(),
        is_nullable: true,
        column_default: None,
        udt_name: "text".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    let mut local_with_table = local.clone();
    local_with_table.tables.insert("users".into(), TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    }); // Table exists but no column -> Drop column

    let diff = compute_diff(&remote_with_col, &local_with_table);
    assert!(diff.is_destructive(), "Dropping a column should be destructive");

    // 3. Safe change (Add table) -> Not Destructive
    let diff = compute_diff(&local, &remote); // Inverse
    assert!(!diff.is_destructive(), "Adding a table should NOT be destructive");
}

#[test]
fn test_policy_comparison_normalized() {
    // Policies with equivalent expressions but different formatting should be considered the same
    let local = PolicyInfo {
        name: "Users can view their own tasks".to_string(),
        cmd: "SELECT".to_string(),
        roles: vec!["public".to_string()],
        qual: Some("auth.uid() = user_id".to_string()),
        with_check: None,
    };

    // Remote might have extra parentheses or different spacing
    let remote = PolicyInfo {
        name: "Users can view their own tasks".to_string(),
        cmd: "SELECT".to_string(),
        roles: vec!["public".to_string()],
        qual: Some("(auth.uid() = user_id)".to_string()),
        with_check: None,
    };

    // These should NOT differ (the expressions are equivalent)
    assert!(!tables::policies_differ(&local, &remote), 
        "Policies with equivalent expressions should not differ");

    // But different commands should differ
    let remote_different_cmd = PolicyInfo {
        name: "Users can view their own tasks".to_string(),
        cmd: "INSERT".to_string(),
        roles: vec!["public".to_string()],
        qual: Some("auth.uid() = user_id".to_string()),
        with_check: None,
    };

    assert!(tables::policies_differ(&local, &remote_different_cmd),
        "Policies with different commands should differ");
}

#[test]
fn test_policy_comparison_with_subquery() {
    // Reproduces the exact issue: PostgreSQL rewrites policy expressions with subqueries,
    // adding table prefixes to column names and parentheses around WHERE conditions.
    // LOCAL: character_id IN (SELECT id FROM "public"."characters" WHERE user_id = auth.uid())
    // REMOTE: (character_id IN ( SELECT characters.id FROM characters WHERE (characters.user_id = auth.uid())))
    let local = PolicyInfo {
        name: "Users can view own character slots".to_string(),
        cmd: "SELECT".to_string(),
        roles: vec!["authenticated".to_string()],
        qual: Some("character_id IN (SELECT id FROM \"public\".\"characters\" WHERE user_id = auth.uid())".to_string()),
        with_check: None,
    };

    let remote = PolicyInfo {
        name: "Users can view own character slots".to_string(),
        cmd: "SELECT".to_string(),
        roles: vec!["authenticated".to_string()],
        qual: Some("(character_id IN ( SELECT characters.id\n   FROM characters\n  WHERE (characters.user_id = auth.uid())))".to_string()),
        with_check: None,
    };

    // These should NOT differ - they are semantically equivalent
    assert!(!tables::policies_differ(&local, &remote), 
        "Policies with equivalent subquery expressions should not differ despite PostgreSQL's rewriting");
}

// ============================================================================
// Tests for Default Object Exclusion (Prevent dropping Supabase system objects)
// ============================================================================

#[test]
fn test_default_roles_excluded_from_diff() {
    use crate::defaults;

    let mut remote = DbSchema::new();
    let local = DbSchema::new();

    // Add default Supabase roles to remote (these exist on every Supabase project)
    for role_name in defaults::DEFAULT_ROLES {
        remote.roles.insert(
            role_name.to_string(),
            crate::schema::RoleInfo {
                name: role_name.to_string(),
                superuser: false,
                create_db: false,
                create_role: false,
                inherit: true,
                login: true,
                replication: false,
                bypass_rls: false,
                connection_limit: -1,
                valid_until: None,
                password: None,
            },
        );
    }

    // Local schema has no roles defined
    let diff = compute_diff(&remote, &local);

    // Default roles should NOT appear in roles_to_drop
    for role_name in defaults::DEFAULT_ROLES {
        assert!(
            !diff.roles_to_drop.contains(&role_name.to_string()),
            "Default role '{}' should NOT be dropped",
            role_name
        );
    }
}

#[test]
fn test_pg_prefixed_roles_excluded_from_diff() {
    use crate::defaults;

    let mut remote = DbSchema::new();
    let local = DbSchema::new();

    // Add pg_* prefixed roles (PostgreSQL system roles)
    let pg_roles = ["pg_read_all_data", "pg_write_all_data", "pg_monitor"];
    for role_name in pg_roles {
        remote.roles.insert(
            role_name.to_string(),
            crate::schema::RoleInfo {
                name: role_name.to_string(),
                superuser: false,
                create_db: false,
                create_role: false,
                inherit: true,
                login: false,
                replication: false,
                bypass_rls: false,
                connection_limit: -1,
                valid_until: None,
                password: None,
            },
        );
    }

    let diff = compute_diff(&remote, &local);

    // pg_* roles should be filtered by is_default_role()
    for role_name in pg_roles {
        assert!(
            defaults::is_default_role(role_name),
            "pg_* role '{}' should be recognized as default",
            role_name
        );
    }

    // And should not appear in diff
    assert!(
        diff.roles_to_drop.is_empty(),
        "pg_* roles should not appear in roles_to_drop"
    );
}

#[test]
fn test_default_extensions_excluded_from_diff() {
    use crate::defaults;

    let mut remote = DbSchema::new();
    let local = DbSchema::new();

    // Add default Supabase extensions to remote
    for ext_name in defaults::DEFAULT_EXTENSIONS {
        remote.extensions.insert(
            ext_name.to_string(),
            crate::schema::ExtensionInfo {
                name: ext_name.to_string(),
                version: Some("1.0".to_string()),
                schema: Some("extensions".to_string()),
            },
        );
    }

    let diff = compute_diff(&remote, &local);

    // Default extensions should NOT appear in extensions_to_drop
    for ext_name in defaults::DEFAULT_EXTENSIONS {
        assert!(
            !diff.extensions_to_drop.contains(&ext_name.to_string()),
            "Default extension '{}' should NOT be dropped",
            ext_name
        );
    }
}

#[test]
fn test_custom_roles_can_be_dropped() {
    let mut remote = DbSchema::new();
    let local = DbSchema::new();

    // Add a custom (non-default) role
    remote.roles.insert(
        "my_app_admin".to_string(),
        crate::schema::RoleInfo {
            name: "my_app_admin".to_string(),
            superuser: false,
            create_db: false,
            create_role: false,
            inherit: true,
            login: true,
            replication: false,
            bypass_rls: false,
            connection_limit: -1,
            valid_until: None,
            password: None,
        },
    );

    let diff = compute_diff(&remote, &local);

    // Custom role SHOULD appear in roles_to_drop
    assert!(
        diff.roles_to_drop.contains(&"my_app_admin".to_string()),
        "Custom role 'my_app_admin' should be marked for dropping"
    );
}

#[test]
fn test_custom_extensions_can_be_dropped() {
    let mut remote = DbSchema::new();
    let local = DbSchema::new();

    // Add a custom (non-default) extension
    remote.extensions.insert(
        "postgis".to_string(),
        crate::schema::ExtensionInfo {
            name: "postgis".to_string(),
            version: Some("3.0".to_string()),
            schema: Some("public".to_string()),
        },
    );

    let diff = compute_diff(&remote, &local);

    // Custom extension SHOULD appear in extensions_to_drop
    assert!(
        diff.extensions_to_drop.contains(&"postgis".to_string()),
        "Custom extension 'postgis' should be marked for dropping"
    );
}

// ============================================================================
// Tests for Destructive Change Detection
// ============================================================================

#[test]
fn test_type_change_is_destructive() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    remote_table.columns.insert(
        "data".into(),
        ColumnInfo {
            column_name: "data".into(),
            data_type: "text".into(),
            is_nullable: true,
            column_default: None,
            udt_name: "text".into(),
            is_primary_key: false,
            is_unique: false,
            is_identity: false,
            identity_generation: None,
        is_generated: false,
        generation_expression: None,
            collation: None,
            enum_name: None,
            is_array: false,
            comment: None,
        },
    );

    let mut local_table = remote_table.clone();
    // Change the type from text to integer (destructive!)
    local_table.columns.insert(
        "data".into(),
        ColumnInfo {
            column_name: "data".into(),
            data_type: "integer".into(),
            is_nullable: true,
            column_default: None,
            udt_name: "int4".into(),
            is_primary_key: false,
            is_unique: false,
            is_identity: false,
            identity_generation: None,
        is_generated: false,
        generation_expression: None,
            collation: None,
            enum_name: None,
            is_array: false,
            comment: None,
        },
    );

    remote.tables.insert("\"public\".\"users\"".into(), remote_table);
    local.tables.insert("\"public\".\"users\"".into(), local_table);

    let diff = compute_diff(&remote, &local);
    assert!(
        diff.is_destructive(),
        "Type change from text to integer should be destructive"
    );
}

#[test]
fn test_enum_drop_is_destructive() {
    let mut remote = DbSchema::new();
    let local = DbSchema::new();

    remote.enums.insert(
        "\"public\".\"status\"".to_string(),
        EnumInfo {
            schema: "public".to_string(),
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()], extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert!(
        diff.is_destructive(),
        "Dropping an enum type should be destructive"
    );
}

#[test]
fn test_add_column_is_not_destructive() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let mut local_table = remote_table.clone();
    local_table.columns.insert(
        "email".into(),
        ColumnInfo {
            column_name: "email".into(),
            data_type: "text".into(),
            is_nullable: true,
            column_default: None,
            udt_name: "text".into(),
            is_primary_key: false,
            is_unique: false,
            is_identity: false,
            identity_generation: None,
        is_generated: false,
        generation_expression: None,
            collation: None,
            enum_name: None,
            is_array: false,
            comment: None,
        },
    );

    remote.tables.insert("users".into(), remote_table);
    local.tables.insert("users".into(), local_table);

    let diff = compute_diff(&remote, &local);
    assert!(
        !diff.is_destructive(),
        "Adding a column should NOT be destructive"
    );
}

#[test]
fn test_enum_add_value_is_not_destructive() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    remote.enums.insert(
        "\"public\".\"status\"".to_string(),
        EnumInfo {
            schema: "public".to_string(),
            name: "status".to_string(),
            values: vec!["active".to_string(), "inactive".to_string()], extension: None,
        },
    );

    local.enums.insert(
        "\"public\".\"status\"".to_string(),
        EnumInfo {
            schema: "public".to_string(),
            name: "status".to_string(),
            values: vec![
                "active".to_string(),
                "inactive".to_string(),
                "pending".to_string(),
            ],
            extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert!(
        !diff.is_destructive(),
        "Adding a value to enum should NOT be destructive"
    );
}

#[test]
fn test_create_function_is_not_destructive() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    local.functions.insert(
        "\"public\".\"my_function\"()".to_string(),
        FunctionInfo {
            schema: "public".to_string(),
            name: "my_function".to_string(),
            args: vec![],
            return_type: "void".to_string(),
            language: "plpgsql".to_string(),
            definition: "BEGIN END;".to_string(),
            volatility: None,
            is_strict: false,
            security_definer: false,
            config_params: vec![],
            grants: vec![], extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert!(
        !diff.is_destructive(),
        "Creating a function should NOT be destructive"
    );
}

// ============================================================================
// End-to-End Test: Full Diff Flow
// ============================================================================

#[test]
fn test_full_schema_diff_does_not_drop_system_objects() {
    use crate::defaults;

    // Simulate a remote schema with Supabase system objects
    let mut remote = DbSchema::new();

    // Add system roles
    for role_name in defaults::DEFAULT_ROLES {
        remote.roles.insert(
            role_name.to_string(),
            crate::schema::RoleInfo {
                name: role_name.to_string(),
                superuser: false,
                create_db: false,
                create_role: false,
                inherit: true,
                login: true,
                replication: false,
                bypass_rls: false,
                connection_limit: -1,
                valid_until: None,
                password: None,
            },
        );
    }

    // Add system extensions
    for ext_name in defaults::DEFAULT_EXTENSIONS {
        remote.extensions.insert(
            ext_name.to_string(),
            crate::schema::ExtensionInfo {
                name: ext_name.to_string(),
                version: Some("1.0".to_string()),
                schema: Some("extensions".to_string()),
            },
        );
    }

    // Add a user table
    let mut users_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    users_table.columns.insert(
        "id".into(),
        ColumnInfo {
            column_name: "id".into(),
            data_type: "uuid".into(),
            is_nullable: false,
            column_default: Some("gen_random_uuid()".into()),
            udt_name: "uuid".into(),
            is_primary_key: true,
            is_unique: true,
            is_identity: false,
            identity_generation: None,
        is_generated: false,
        generation_expression: None,
            collation: None,
            enum_name: None,
            is_array: false,
            comment: None,
        },
    );
    remote.tables.insert("\"public\".\"users\"".into(), users_table.clone());

    // Local schema: same user table, no system objects defined (typical local schema file)
    let mut local = DbSchema::new();
    local.tables.insert("\"public\".\"users\"".into(), users_table);

    // Compute diff
    let diff = compute_diff(&remote, &local);

    // Verify no system objects are dropped
    assert!(
        diff.roles_to_drop.is_empty(),
        "No roles should be dropped: {:?}",
        diff.roles_to_drop
    );
    assert!(
        diff.extensions_to_drop.is_empty(),
        "No extensions should be dropped: {:?}",
        diff.extensions_to_drop
    );

    // Verify the diff is not destructive
    assert!(!diff.is_destructive(), "Diff should not be destructive");

    // Verify no changes to tables
    assert!(diff.tables_to_create.is_empty());
    assert!(diff.tables_to_drop.is_empty());
    assert!(diff.table_changes.is_empty());
}

// ============================================================================
// Additional Diff Tests for Full Postgres Feature Coverage
// ============================================================================

#[test]
fn test_view_create() {
    let remote = DbSchema::new();
    let mut local = DbSchema::new();

    local.views.insert(
        "\"public\".\"user_stats\"".to_string(),
        crate::schema::ViewInfo {
            schema: "public".to_string(),
            name: "user_stats".to_string(),
            definition: "SELECT id, COUNT(*) FROM users GROUP BY id".to_string(),
            is_materialized: false,
            columns: vec![],
            indexes: vec![],
            comment: None,
            with_options: vec![],
            check_option: None, extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.views_to_create.len(), 1);
    assert_eq!(diff.views_to_create[0].name, "user_stats");
}

#[test]
fn test_view_drop() {
    let mut remote = DbSchema::new();
    let local = DbSchema::new();

    remote.views.insert(
        "\"public\".\"old_view\"".to_string(),
        crate::schema::ViewInfo {
            schema: "public".to_string(),
            name: "old_view".to_string(),
            definition: "SELECT 1".to_string(),
            is_materialized: false,
            columns: vec![],
            indexes: vec![],
            comment: None,
            with_options: vec![],
            check_option: None, extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.views_to_drop.len(), 1);
    assert!(diff.views_to_drop.contains(&"\"public\".\"old_view\"".to_string()));
}

#[test]
fn test_view_update() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    remote.views.insert(
        "\"public\".\"stats\"".to_string(),
        crate::schema::ViewInfo {
            schema: "public".to_string(),
            name: "stats".to_string(),
            definition: "SELECT id FROM users".to_string(),
            is_materialized: false,
            columns: vec![],
            indexes: vec![],
            comment: None,
            with_options: vec![],
            check_option: None, extension: None,
        },
    );

    local.views.insert(
        "\"public\".\"stats\"".to_string(),
        crate::schema::ViewInfo {
            schema: "public".to_string(),
            name: "stats".to_string(),
            definition: "SELECT id, name FROM users".to_string(), // Changed
            is_materialized: false,
            columns: vec![],
            indexes: vec![],
            comment: None,
            with_options: vec![],
            check_option: None, extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.views_to_update.len(), 1);
    assert_eq!(diff.views_to_update[0].name, "stats");
}

#[test]
fn test_materialized_view_create() {
    let remote = DbSchema::new();
    let mut local = DbSchema::new();

    local.views.insert(
        "\"public\".\"cached_stats\"".to_string(),
        crate::schema::ViewInfo {
            schema: "public".to_string(),
            name: "cached_stats".to_string(),
            definition: "SELECT * FROM users".to_string(),
            is_materialized: true,
            columns: vec![],
            indexes: vec![],
            comment: None,
            with_options: vec![],
            check_option: None, extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.views_to_create.len(), 1);
    assert!(diff.views_to_create[0].is_materialized);
}

#[test]
fn test_sequence_create() {
    let remote = DbSchema::new();
    let mut local = DbSchema::new();

    local.sequences.insert(
        "\"public\".\"order_seq\"".to_string(),
        crate::schema::SequenceInfo {
            schema: "public".to_string(),
            name: "order_seq".to_string(),
            data_type: "bigint".to_string(),
            start_value: 1,
            min_value: 1,
            max_value: 9223372036854775807,
            increment: 1,
            cycle: false,
            cache_size: 1,
            owned_by: None, extension: None,
            comment: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.sequences_to_create.len(), 1);
    assert_eq!(diff.sequences_to_create[0].name, "order_seq");
}

#[test]
fn test_sequence_drop() {
    let mut remote = DbSchema::new();
    let local = DbSchema::new();

    remote.sequences.insert(
        "\"public\".\"old_seq\"".to_string(),
        crate::schema::SequenceInfo {
            schema: "public".to_string(),
            name: "old_seq".to_string(),
            data_type: "bigint".to_string(),
            start_value: 1,
            min_value: 1,
            max_value: 9223372036854775807,
            increment: 1,
            cycle: false,
            cache_size: 1,
            owned_by: None, extension: None,
            comment: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.sequences_to_drop.len(), 1);
}

#[test]
fn test_sequence_update() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    remote.sequences.insert(
        "\"public\".\"my_seq\"".to_string(),
        crate::schema::SequenceInfo {
            schema: "public".to_string(),
            name: "my_seq".to_string(),
            data_type: "bigint".to_string(),
            start_value: 1,
            min_value: 1,
            max_value: 9223372036854775807,
            increment: 1,
            cycle: false,
            cache_size: 1,
            owned_by: None, extension: None,
            comment: None,
        },
    );

    local.sequences.insert(
        "\"public\".\"my_seq\"".to_string(),
        crate::schema::SequenceInfo {
            schema: "public".to_string(),
            name: "my_seq".to_string(),
            data_type: "bigint".to_string(),
            start_value: 1,
            min_value: 1,
            max_value: 9223372036854775807,
            increment: 5, // Changed increment
            cycle: false,
            cache_size: 1,
            owned_by: None, extension: None,
            comment: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.sequences_to_update.len(), 1);
}

#[test]
fn test_function_update() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    remote.functions.insert(
        "\"public\".\"my_func\"()".to_string(),
        FunctionInfo {
            schema: "public".to_string(),
            name: "my_func".to_string(),
            args: vec![],
            return_type: "integer".to_string(),
            language: "sql".to_string(),
            definition: "SELECT 1".to_string(),
            volatility: None,
            is_strict: false,
            security_definer: false,
            config_params: vec![],
            grants: vec![], extension: None,
        },
    );

    local.functions.insert(
        "\"public\".\"my_func\"()".to_string(),
        FunctionInfo {
            schema: "public".to_string(),
            name: "my_func".to_string(),
            args: vec![],
            return_type: "integer".to_string(),
            language: "sql".to_string(),
            definition: "SELECT 2".to_string(), // Changed definition
            volatility: None,
            is_strict: false,
            security_definer: false,
            config_params: vec![],
            grants: vec![], extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.functions_to_update.len(), 1);
    assert_eq!(diff.functions_to_update[0].name, "my_func");
}

#[test]
fn test_domain_create() {
    let remote = DbSchema::new();
    let mut local = DbSchema::new();

    local.domains.insert(
        "\"public\".\"email_addr\"".to_string(),
        crate::schema::DomainInfo {
            schema: "public".to_string(),
            name: "email_addr".to_string(),
            base_type: "text".to_string(),
            default_value: None,
            is_not_null: false,
            check_constraints: vec![], extension: None,
            collation: None,
            comment: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.domains_to_create.len(), 1);
    assert_eq!(diff.domains_to_create[0].name, "email_addr");
}

#[test]
fn test_domain_drop() {
    let mut remote = DbSchema::new();
    let local = DbSchema::new();

    remote.domains.insert(
        "\"public\".\"old_domain\"".to_string(),
        crate::schema::DomainInfo {
            schema: "public".to_string(),
            name: "old_domain".to_string(),
            base_type: "integer".to_string(),
            default_value: None,
            is_not_null: false,
            check_constraints: vec![], extension: None,
            collation: None,
            comment: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.domains_to_drop.len(), 1);
}

#[test]
fn test_composite_type_create() {
    let remote = DbSchema::new();
    let mut local = DbSchema::new();

    local.composite_types.insert(
        "\"public\".\"address\"".to_string(),
        crate::schema::CompositeTypeInfo {
            schema: "public".to_string(),
            name: "address".to_string(),
            attributes: vec![
                crate::schema::CompositeTypeAttribute {
                    name: "street".to_string(),
                    data_type: "text".to_string(),
                    collation: None,
                },
            ],
            comment: None,
            extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.composite_types_to_create.len(), 1);
}

#[test]
fn test_composite_type_drop() {
    let mut remote = DbSchema::new();
    let local = DbSchema::new();

    remote.composite_types.insert(
        "\"public\".\"old_type\"".to_string(),
        crate::schema::CompositeTypeInfo {
            schema: "public".to_string(),
            name: "old_type".to_string(),
            attributes: vec![],
            comment: None,
            extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.composite_types_to_drop.len(), 1);
}

#[test]
fn test_extension_create() {
    let remote = DbSchema::new();
    let mut local = DbSchema::new();

    local.extensions.insert(
        "postgis".to_string(),
        crate::schema::ExtensionInfo {
            name: "postgis".to_string(),
            version: Some("3.0".to_string()),
            schema: Some("public".to_string()),
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.extensions_to_create.len(), 1);
    assert_eq!(diff.extensions_to_create[0].name, "postgis");
}

#[test]
fn test_role_create() {
    let remote = DbSchema::new();
    let mut local = DbSchema::new();

    local.roles.insert(
        "app_user".to_string(),
        crate::schema::RoleInfo {
            name: "app_user".to_string(),
            superuser: false,
            create_db: false,
            create_role: false,
            inherit: true,
            login: true,
            replication: false,
            bypass_rls: false,
            connection_limit: -1,
            valid_until: None,
            password: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.roles_to_create.len(), 1);
    assert_eq!(diff.roles_to_create[0].name, "app_user");
}

#[test]
fn test_role_update() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    remote.roles.insert(
        "app_user".to_string(),
        crate::schema::RoleInfo {
            name: "app_user".to_string(),
            superuser: false,
            create_db: false,
            create_role: false,
            inherit: true,
            login: true,
            replication: false,
            bypass_rls: false,
            connection_limit: -1,
            valid_until: None,
            password: None,
        },
    );

    local.roles.insert(
        "app_user".to_string(),
        crate::schema::RoleInfo {
            name: "app_user".to_string(),
            superuser: false,
            create_db: true, // Changed
            create_role: false,
            inherit: true,
            login: true,
            replication: false,
            bypass_rls: false,
            connection_limit: -1,
            valid_until: None,
            password: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.roles_to_update.len(), 1);
}

#[test]
fn test_column_default_change() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    remote_table.columns.insert("age".into(), ColumnInfo {
        column_name: "age".into(),
        data_type: "integer".into(),
        is_nullable: true,
        column_default: None, // No default
        udt_name: "int4".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    let mut local_table = remote_table.clone();
    local_table.columns.insert("age".into(), ColumnInfo {
        column_name: "age".into(),
        data_type: "integer".into(),
        is_nullable: true,
        column_default: Some("18".into()), // Added default
        udt_name: "int4".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    remote.tables.insert("users".into(), remote_table);
    local.tables.insert("users".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("users").unwrap();
    assert_eq!(table_diff.columns_to_modify.len(), 1);
    assert!(table_diff.columns_to_modify[0].changes.default_change.is_some());
}

#[test]
fn test_identity_column_change() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "items".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    remote_table.columns.insert("id".into(), ColumnInfo {
        column_name: "id".into(),
        data_type: "integer".into(),
        is_nullable: false,
        column_default: None,
        udt_name: "int4".into(),
        is_primary_key: true,
        is_unique: true,
        is_identity: false, // Not identity
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    let mut local_table = remote_table.clone();
    local_table.columns.insert("id".into(), ColumnInfo {
        column_name: "id".into(),
        data_type: "integer".into(),
        is_nullable: false,
        column_default: None,
        udt_name: "int4".into(),
        is_primary_key: true,
        is_unique: true,
        is_identity: true, // Now identity
        identity_generation: Some("ALWAYS".into()),
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    remote.tables.insert("items".into(), remote_table);
    local.tables.insert("items".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("items").unwrap();
    assert_eq!(table_diff.columns_to_modify.len(), 1);
    assert!(table_diff.columns_to_modify[0].changes.identity_change.is_some());
}

#[test]
fn test_collation_change() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "data".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    remote_table.columns.insert("name".into(), ColumnInfo {
        column_name: "name".into(),
        data_type: "text".into(),
        is_nullable: true,
        column_default: None,
        udt_name: "text".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None, // No collation
        enum_name: None,
        is_array: false,
        comment: None,
    });

    let mut local_table = remote_table.clone();
    local_table.columns.insert("name".into(), ColumnInfo {
        column_name: "name".into(),
        data_type: "text".into(),
        is_nullable: true,
        column_default: None,
        udt_name: "text".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: Some("\"C\"".into()), // Added collation
        enum_name: None,
        is_array: false,
        comment: None,
    });

    remote.tables.insert("data".into(), remote_table);
    local.tables.insert("data".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("data").unwrap();
    assert_eq!(table_diff.columns_to_modify.len(), 1);
    assert!(table_diff.columns_to_modify[0].changes.collation_change.is_some());
}

#[test]
fn test_column_comment_change() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    remote_table.columns.insert("email".into(), ColumnInfo {
        column_name: "email".into(),
        data_type: "text".into(),
        is_nullable: true,
        column_default: None,
        udt_name: "text".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    let mut local_table = remote_table.clone();
    local_table.columns.insert("email".into(), ColumnInfo {
        column_name: "email".into(),
        data_type: "text".into(),
        is_nullable: true,
        column_default: None,
        udt_name: "text".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: Some("User email address".into()), // Added comment
    });

    remote.tables.insert("users".into(), remote_table);
    local.tables.insert("users".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("users").unwrap();
    assert_eq!(table_diff.columns_to_modify.len(), 1);
    assert!(table_diff.columns_to_modify[0].changes.comment_change.is_some());
}

#[test]
fn test_table_comment_change() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let mut local_table = remote_table.clone();
    local_table.comment = Some("Main users table".into());

    remote.tables.insert("users".into(), remote_table);
    local.tables.insert("users".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("users").unwrap();
    assert!(table_diff.comment_change.is_some());
}

#[test]
fn test_foreign_key_add() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let remote_table = TableInfo {
        schema: "public".into(),
        table_name: "posts".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let mut local_table = remote_table.clone();
    local_table.foreign_keys.push(ForeignKeyInfo {
        constraint_name: "fk_user".into(),
        columns: vec!["user_id".into()],
        foreign_schema: "public".into(),
        foreign_table: "users".into(),
        foreign_columns: vec!["id".into()],
        on_delete: "CASCADE".into(),
        on_update: "NO ACTION".into(),
    });

    remote.tables.insert("posts".into(), remote_table);
    local.tables.insert("posts".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("posts").unwrap();
    assert_eq!(table_diff.foreign_keys_to_create.len(), 1);
    assert_eq!(table_diff.foreign_keys_to_create[0].constraint_name, "fk_user");
}

#[test]
fn test_foreign_key_drop() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "posts".into(),
        columns: HashMap::new(),
        foreign_keys: vec![ForeignKeyInfo {
            constraint_name: "fk_user".into(),
            columns: vec!["user_id".into()],
            foreign_schema: "public".into(),
            foreign_table: "users".into(),
            foreign_columns: vec!["id".into()],
            on_delete: "CASCADE".into(),
            on_update: "NO ACTION".into(),
        }],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let local_table = TableInfo {
        schema: "public".into(),
        table_name: "posts".into(),
        columns: HashMap::new(),
        foreign_keys: vec![], // FK removed
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    remote.tables.insert("posts".into(), remote_table);
    local.tables.insert("posts".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("posts").unwrap();
    assert_eq!(table_diff.foreign_keys_to_drop.len(), 1);
}

#[test]
fn test_trigger_create() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let remote_table = TableInfo {
        schema: "public".into(),
        table_name: "events".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let mut local_table = remote_table.clone();
    local_table.triggers.push(TriggerInfo {
        name: "audit_trigger".into(),
        events: vec!["INSERT".into(), "UPDATE".into()],
        timing: "AFTER".into(),
        orientation: "ROW".into(),
        function_name: "audit_func".into(),
        when_clause: None,
    });

    remote.tables.insert("events".into(), remote_table);
    local.tables.insert("events".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("events").unwrap();
    assert_eq!(table_diff.triggers_to_create.len(), 1);
}

#[test]
fn test_trigger_drop() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let remote_table = TableInfo {
        schema: "public".into(),
        table_name: "events".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![TriggerInfo {
            name: "old_trigger".into(),
            events: vec!["INSERT".into()],
            timing: "BEFORE".into(),
            orientation: "ROW".into(),
            function_name: "old_func".into(),
            when_clause: None,
        }],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let local_table = TableInfo {
        schema: "public".into(),
        table_name: "events".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![], // Trigger removed
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    remote.tables.insert("events".into(), remote_table);
    local.tables.insert("events".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("events").unwrap();
    assert_eq!(table_diff.triggers_to_drop.len(), 1);
}

#[test]
fn test_index_with_expression() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let remote_table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let mut local_table = remote_table.clone();
    local_table.indexes.push(IndexInfo {
        index_name: "idx_lower_email".into(),
        columns: vec![],
        is_unique: false,
        is_primary: false,
        owning_constraint: None,
        index_method: "btree".into(),
        where_clause: None,
        expressions: vec!["lower(email)".into()],
    });

    remote.tables.insert("users".into(), remote_table);
    local.tables.insert("users".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("users").unwrap();
    assert_eq!(table_diff.indexes_to_create.len(), 1);
    assert!(!table_diff.indexes_to_create[0].expressions.is_empty());
}

#[test]
fn test_policy_create() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let remote_table = TableInfo {
        schema: "public".into(),
        table_name: "posts".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: true,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let mut local_table = remote_table.clone();
    local_table.policies.push(PolicyInfo {
        name: "select_own".into(),
        cmd: "SELECT".into(),
        roles: vec!["public".into()],
        qual: Some("user_id = auth.uid()".into()),
        with_check: None,
    });

    remote.tables.insert("posts".into(), remote_table);
    local.tables.insert("posts".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("posts").unwrap();
    assert_eq!(table_diff.policies_to_create.len(), 1);
}

#[test]
fn test_policy_drop() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let remote_table = TableInfo {
        schema: "public".into(),
        table_name: "posts".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: true,
        policies: vec![PolicyInfo {
            name: "old_policy".into(),
            cmd: "SELECT".into(),
            roles: vec!["public".into()],
            qual: Some("true".into()),
            with_check: None,
        }],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let local_table = TableInfo {
        schema: "public".into(),
        table_name: "posts".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: true,
        policies: vec![], // Policy removed
        check_constraints: vec![], extension: None,
        comment: None,
    };

    remote.tables.insert("posts".into(), remote_table);
    local.tables.insert("posts".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("posts").unwrap();
    assert_eq!(table_diff.policies_to_drop.len(), 1);
}

#[test]
fn test_rls_enable() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let remote_table = TableInfo {
        schema: "public".into(),
        table_name: "posts".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let mut local_table = remote_table.clone();
    local_table.rls_enabled = true;

    remote.tables.insert("posts".into(), remote_table);
    local.tables.insert("posts".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("posts").unwrap();
    assert_eq!(table_diff.rls_change, Some(true));
}

#[test]
fn test_rls_disable() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let remote_table = TableInfo {
        schema: "public".into(),
        table_name: "posts".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: true,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    let mut local_table = remote_table.clone();
    local_table.rls_enabled = false;

    remote.tables.insert("posts".into(), remote_table);
    local.tables.insert("posts".into(), local_table);

    let diff = compute_diff(&remote, &local);
    let table_diff = diff.table_changes.get("posts").unwrap();
    assert_eq!(table_diff.rls_change, Some(false));
}

#[test]
fn test_function_drop() {
    let mut remote = DbSchema::new();
    let local = DbSchema::new();

    remote.functions.insert(
        "\"public\".\"old_func\"()".to_string(),
        FunctionInfo {
            schema: "public".to_string(),
            name: "old_func".to_string(),
            args: vec![],
            return_type: "void".to_string(),
            language: "plpgsql".to_string(),
            definition: "BEGIN END;".to_string(),
            volatility: None,
            is_strict: false,
            security_definer: false,
            config_params: vec![],
            grants: vec![], extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert_eq!(diff.functions_to_drop.len(), 1);
}

#[test]
fn test_schema_diff_is_empty() {
    let remote = DbSchema::new();
    let local = DbSchema::new();

    let diff = compute_diff(&remote, &local);
    assert!(diff.is_empty());
}

#[test]
fn test_table_diff_is_empty() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };

    remote.tables.insert("users".into(), table.clone());
    local.tables.insert("users".into(), table);

    let diff = compute_diff(&remote, &local);
    assert!(diff.table_changes.is_empty());
}

#[test]
fn test_function_definition_normalization() {
    // Test that functions with equivalent definitions but different formatting
    // (dollar quoting, quoted identifiers) are NOT marked as updates
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    // Remote format: uses $function$ dollar quoting and unquoted identifiers
    let remote_definition = r#"begin
  return query
  select r.id, r.slug, r.name
  from public.recipes r
  where (
    select count(*)
    from public.recipe_ingredients ri
    where ri.recipe_id = r.id
  ) = array_length(ingredient_ids, 1);
end;"#;

    // Local format: uses $$ dollar quoting and quoted identifiers
    // This is what pg_dump / schema introspection often produces differently
    let local_definition = r#"begin
  return query
  select r.id, r.slug, r.name
  from public.recipes r
  where (
    select count(*)
    from public.recipe_ingredients ri
    where ri.recipe_id = r.id
  ) = array_length(ingredient_ids, 1);
end;"#;

    remote.functions.insert(
        "\"public\".\"find_recipes_by_ingredients\"(uuid[])".to_string(),
        FunctionInfo {
            schema: "public".to_string(),
            name: "find_recipes_by_ingredients".to_string(),
            args: vec![FunctionArg {
                name: "ingredient_ids".to_string(),
                type_: "uuid[]".to_string(),
                mode: None,
                default_value: None,
            }],
            return_type: "TABLE(id uuid, slug text, name text)".to_string(),
            language: "plpgsql".to_string(),
            definition: remote_definition.to_string(),
            volatility: None,
            is_strict: false,
            security_definer: false,
            config_params: vec![],
            grants: vec![], extension: None,
        },
    );

    local.functions.insert(
        "\"public\".\"find_recipes_by_ingredients\"(uuid[])".to_string(),
        FunctionInfo {
            schema: "public".to_string(),
            name: "find_recipes_by_ingredients".to_string(),
            args: vec![FunctionArg {
                name: "ingredient_ids".to_string(),
                type_: "uuid[]".to_string(),
                mode: None,
                default_value: None,
            }],
            return_type: "table(id uuid, slug text, name text)".to_string(), // Different case
            language: "plpgsql".to_string(),
            definition: local_definition.to_string(),
            volatility: None,
            is_strict: false,
            security_definer: false,
            config_params: vec![],
            grants: vec![], extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);
    assert!(
        diff.functions_to_update.is_empty(),
        "Functions with equivalent definitions but different formatting should NOT be marked for update"
    );
}

#[test]
fn test_function_with_different_dollar_quotes_normalization() {
    // Test specifically for $function$ vs $$ dollar quoting
    use super::utils::normalize_function_definition;

    let remote_def = "$function$SELECT 1$function$";
    let local_def = "$$SELECT 1$$";

    let normalized_remote = normalize_function_definition(remote_def);
    let normalized_local = normalize_function_definition(local_def);

    assert_eq!(
        normalized_remote, normalized_local,
        "Different dollar quote styles should normalize to the same value"
    );
}

#[test]
fn test_function_with_quoted_identifiers_normalization() {
    // Test that quoted identifiers are stripped
    use super::utils::normalize_function_definition;

    let remote_def = "SELECT * FROM public.users";
    let local_def = r#"SELECT * FROM "public"."users""#;

    let normalized_remote = normalize_function_definition(remote_def);
    let normalized_local = normalize_function_definition(local_def);

    assert_eq!(
        normalized_remote, normalized_local,
        "Quoted and unquoted identifiers should normalize to the same value"
    );
}

#[test]
fn test_view_definition_normalization() {
    use super::utils::normalize_view_definition;

    // Local definition might include quotes around identifiers
    let local_def = r#"SELECT r.id, r.slug, r.name FROM "public"."recipes" r"#;
    
    // Remote definition from pg_get_viewdef doesn't include quotes
    let remote_def = r#"SELECT r.id, r.slug, r.name FROM public.recipes r"#;

    let normalized_local = normalize_view_definition(local_def);
    let normalized_remote = normalize_view_definition(remote_def);

    assert_eq!(
        normalized_local, normalized_remote,
        "View definitions with quoted vs unquoted identifiers should normalize to the same value.\nLocal: {}\nRemote: {}",
        normalized_local, normalized_remote
    );
}

#[test]
fn test_view_definition_strips_create_view_prefix() {
    use super::utils::normalize_view_definition;

    // Local might have full CREATE VIEW statement
    let with_create = r#"CREATE OR REPLACE VIEW "public"."my_view" AS SELECT id FROM users"#;
    
    // Remote only has the SELECT
    let just_select = r#"SELECT id FROM users"#;

    let normalized_with_create = normalize_view_definition(with_create);
    let normalized_just_select = normalize_view_definition(just_select);

    assert_eq!(
        normalized_with_create, normalized_just_select,
        "CREATE VIEW prefix should be stripped during normalization.\nWith CREATE: {}\nJust SELECT: {}",
        normalized_with_create, normalized_just_select
    );
}

#[test]
fn test_view_diff_normalization_coalesce_cast() {
    let local_def = "SELECT i.id, COALESCE(SUM(s.quantity), 0) AS total_quantity_sold FROM items i";
    let remote_def = "SELECT i.id, COALESCE(SUM(s.quantity), (0)::bigint) AS total_quantity_sold FROM items i";

    let local = ViewInfo {
        schema: "public".into(),
        name: "test_view".into(),
        definition: local_def.to_string(),
        is_materialized: false,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: None, extension: None,
    };

    let remote = ViewInfo {
        schema: "public".into(),
        name: "test_view".into(),
        definition: remote_def.to_string(),
        is_materialized: false,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: None, extension: None,
    };

    assert!(!super::objects::views_differ(&local, &remote), "Views should be considered identical despite type casting");
}

#[test]
fn test_view_diff_normalization_interval() {
    let local_def = "SELECT * FROM items WHERE created_at > now() - interval '7 days'";
    // Based on user report: remote has (now() - '7 days'::interval) which got mangled to '7 days'erval
    let remote_def = "SELECT * FROM items WHERE created_at > (now() - '7 days'::interval)";

    let local = ViewInfo {
        schema: "public".into(),
        name: "interval_view".into(),
        definition: local_def.to_string(),
        is_materialized: false,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: None, extension: None,
    };

    let remote = ViewInfo {
        schema: "public".into(),
        name: "interval_view".into(),
        definition: remote_def.to_string(),
        is_materialized: false,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: None, extension: None,
    };

    assert!(!super::objects::views_differ(&local, &remote), "Views should be considered identical despite interval syntax differences");
}

#[test]
fn test_view_diff_normalization_complex_parens() {
    // Local: standard format
    // Remote: pg_get_viewdef craziness with extra parens in ON and FILTER
    let local_def = "SELECT count(s.id) FILTER (WHERE s.created_at > now() - interval '7 days') AS sales_last_7_days FROM items i LEFT JOIN item_sales s ON i.id = s.item_id";
    
    // Remote has:
    // 1. FILTER (WHERE (s.created_at > (now() - '7 days'::interval)))
    // 2. ON ((i.id = s.item_id))
    let remote_def = "SELECT count(s.id) FILTER (WHERE (s.created_at > (now() - '7 days'::interval))) AS sales_last_7_days FROM items i LEFT JOIN item_sales s ON ((i.id = s.item_id))";

    let local = ViewInfo {
        schema: "public".into(),
        name: "complex_view".into(),
        definition: local_def.to_string(),
        is_materialized: false,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: None, extension: None,
    };

    let remote = ViewInfo {
        schema: "public".into(),
        name: "complex_view".into(),
        definition: remote_def.to_string(),
        is_materialized: false,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: None, extension: None,
    };

    assert!(!super::objects::views_differ(&local, &remote), "Views should be considered identical despite complex nested parens in JOIN/FILTER");
}

#[test]
fn test_view_diff_normalization_join_on_group_by() {
    // Reproduces the exact issue from bug report:
    // LOCAL: "on i.id = s.item_id group by"
    // REMOTE: "((i.id = s.item_id)))group by" (extra parens, no space before group by)
    let local_def = r#"SELECT i.id AS item_id, i.name AS item_name, i.rarity, COUNT(s.id) AS total_sales, COALESCE(SUM(s.quantity), 0) AS total_quantity_sold, COALESCE(ROUND(AVG(s.price_per_unit)), 0) AS avg_price, COALESCE(MIN(s.price_per_unit), 0) AS min_price, COALESCE(MAX(s.price_per_unit), 0) AS max_price, COALESCE(ROUND(AVG(s.price_per_unit) FILTER (WHERE s.created_at > NOW() - INTERVAL '7 days')), 0) AS avg_price_last_7_days, COALESCE(COUNT(s.id) FILTER (WHERE s.created_at > NOW() - INTERVAL '7 days'), 0) AS sales_last_7_days FROM items i LEFT JOIN item_sales s ON i.id = s.item_id GROUP BY i.id, i.name, i.rarity"#;
    
    // Remote with pg_get_viewdef peculiarities:
    // 1. Extra parens around JOIN: FROM((items i left join item_sales s...
    // 2. Extra parens around ON condition: ON((i.id = s.item_id))
    // 3. No space before GROUP BY: )))group by
    let remote_def = r#"SELECT i.id AS item_id, i.name AS item_name, i.rarity, COUNT(s.id) AS total_sales, COALESCE(SUM(s.quantity), (0)::bigint) AS total_quantity_sold, COALESCE(ROUND(AVG(s.price_per_unit)), (0)::bigint) AS avg_price, COALESCE(MIN(s.price_per_unit), (0)::bigint) AS min_price, COALESCE(MAX(s.price_per_unit), (0)::bigint) AS max_price, COALESCE(ROUND(AVG(s.price_per_unit) FILTER (WHERE (s.created_at > (now() - '7 days'::interval)))), (0)::bigint) AS avg_price_last_7_days, COALESCE(COUNT(s.id) FILTER (WHERE (s.created_at > (now() - '7 days'::interval))), 0) AS sales_last_7_days FROM((items i LEFT JOIN item_sales s ON((i.id = s.item_id))))GROUP BY i.id, i.name, i.rarity"#;

    let local = ViewInfo {
        schema: "public".into(),
        name: "item_price_stats".into(),
        definition: local_def.to_string(),
        is_materialized: false,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: None, extension: None,
    };

    let remote = ViewInfo {
        schema: "public".into(),
        name: "item_price_stats".into(),
        definition: remote_def.to_string(),
        is_materialized: false,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: None, extension: None,
    };

    assert!(!super::objects::views_differ(&local, &remote), "Views should be identical despite pg_get_viewdef's extra parens around JOIN/ON and missing space before GROUP BY");
}
#[test]
fn test_function_param_rename_detection() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    // Remote function: my_func(p_id uuid)
    remote.functions.insert(
        "my_func(uuid)".to_string(),
        FunctionInfo {
            schema: "public".to_string(),
            name: "my_func".to_string(),
            args: vec![FunctionArg {
                name: "p_id".to_string(),
                type_: "uuid".to_string(),
                mode: None,
                default_value: None,
            }],
            return_type: "uuid".to_string(),
            language: "plpgsql".to_string(),
            definition: "BEGIN RETURN p_id; END;".to_string(),
            volatility: Some("VOLATILE".to_string()),
            is_strict: false,
            security_definer: false,
            config_params: vec![],
            grants: vec![], extension: None,
        },
    );

    // Local function: my_func(p_uuid uuid) - Param name changed from p_id to p_uuid
    local.functions.insert(
        "my_func(uuid)".to_string(),
        FunctionInfo {
            schema: "public".to_string(),
            name: "my_func".to_string(),
            args: vec![FunctionArg {
                name: "p_uuid".to_string(), // CHANGED NAME
                type_: "uuid".to_string(),
                mode: None,
                default_value: None,
            }],
            return_type: "uuid".to_string(),
            language: "plpgsql".to_string(),
            definition: "BEGIN RETURN p_uuid; END;".to_string(),
            volatility: Some("VOLATILE".to_string()),
            is_strict: false,
            security_definer: false,
            config_params: vec![],
            grants: vec![], extension: None,
        },
    );

    let diff = compute_diff(&remote, &local);

    // Should NOT be an update (because CREATE OR REPLACE fails on param rename)
    assert!(
        diff.functions_to_update.is_empty(),
        "Function with changed param name should NOT be in functions_to_update"
    );

    // Should be in drop and create
    assert!(
        diff.functions_to_drop.contains(&"my_func(uuid)".to_string()),
        "Function with changed param name should be in functions_to_drop"
    );
    assert!(
        diff.functions_to_create.iter().any(|f| f.name == "my_func" && f.args[0].name == "p_uuid"),
        "Function with changed param name should be in functions_to_create"
    );
}

#[test]
fn test_function_grants_ignore_defaults() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let func_name = "\"public\".\"my_func\"()";
    let definition = "CREATE FUNCTION my_func() RETURNS void LANGUAGE sql AS $$ SELECT 1; $$";

    // maximize similarity for other fields
    let mut func_info = FunctionInfo {
        name: "my_func".to_string(),
        schema: "public".to_string(),
        args: vec![],
        return_type: "void".to_string(),
        language: "sql".to_string(),
        definition: definition.to_string(),
        security_definer: false,
        volatility: None,
        is_strict: false,
        config_params: vec![],
        grants: vec![], extension: None,
    };

    // REMOTE has many grants (authenticated, anon, service_role, postgres, public)
    let mut remote_func = func_info.clone();
    remote_func.grants = vec![
        FunctionGrant { grantee: "authenticated".to_string(), privilege: "EXECUTE".to_string() },
        FunctionGrant { grantee: "anon".to_string(), privilege: "EXECUTE".to_string() },
        FunctionGrant { grantee: "service_role".to_string(), privilege: "EXECUTE".to_string() },
        FunctionGrant { grantee: "postgres".to_string(), privilege: "EXECUTE".to_string() },
        FunctionGrant { grantee: "public".to_string(), privilege: "EXECUTE".to_string() },
    ];
    remote.functions.insert(func_name.to_string(), remote_func);

    // LOCAL only has service_role grant
    let mut local_func = func_info.clone();
    local_func.grants = vec![
        FunctionGrant { grantee: "service_role".to_string(), privilege: "EXECUTE".to_string() },
    ];
    local.functions.insert(func_name.to_string(), local_func);

    let diff = compute_diff(&remote, &local);

    // Should NOT be an update because extra remote grants are ignored if they are defaults
    assert!(
        diff.functions_to_update.is_empty(),
        "Function should NOT be updated when only default grants differ. Updates: {:?}",
        diff.functions_to_update
    );
}

#[test]
fn test_extension_artifact_filtering() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    // Both have the extension
    remote.extensions.insert(
        "pgmq".to_string(),
        crate::schema::ExtensionInfo {
            name: "pgmq".to_string(),
            version: Some("1.0".to_string()),
            schema: Some("public".to_string()),
        },
    );
    local.extensions.insert(
        "pgmq".to_string(),
        crate::schema::ExtensionInfo {
            name: "pgmq".to_string(),
            version: Some("1.0".to_string()),
            schema: Some("public".to_string()),
        },
    );

    // Remote has a function belonging to the extension
    remote.functions.insert(
        "\"public\".\"pgmq_archive\"(bigint)".to_string(),
        FunctionInfo {
            schema: "public".to_string(),
            name: "pgmq_archive".to_string(),
            args: vec![crate::schema::FunctionArg {
                name: "msg_id".to_string(),
                type_: "bigint".to_string(),
                mode: None,
                default_value: None,
            }],
            return_type: "boolean".to_string(),
            language: "plpgsql".to_string(),
            definition: "BEGIN END;".to_string(),
            volatility: None,
            is_strict: false,
            security_definer: false,
            config_params: vec![],
            grants: vec![],
            extension: Some("pgmq".to_string()), // Owned by extension
        },
    );

    // Local does NOT have the function explicitly defined (it comes with the extension in theory)
    // But importantly, it is NOT in local.functions map.

    let diff = compute_diff(&remote, &local);

    // Should NOT drop the extension
    assert!(diff.extensions_to_drop.is_empty(), "Should not drop extension. Drops: {:?}", diff.extensions_to_drop);

    // Should NOT drop the function because it belongs to the extension
    assert!(
        diff.functions_to_drop.is_empty(),
        "Should not drop extension-owned function 'pgmq_archive', but got: {:?}",
        diff.functions_to_drop
    );
}

#[test]
fn test_expression_only_index_no_diff() {
    // Expression-only indexes (like coalesce(...)) should not produce spurious diffs
    // when both local and remote have the same index with equivalent expressions
    let local_idx = IndexInfo {
        index_name: "role_bindings_member_unique_idx".to_string(),
        columns: vec![],
        is_unique: true,
        is_primary: false,
        owning_constraint: None,
        index_method: "btree".to_string(),
        where_clause: Some("principal_member_id IS NOT NULL".to_string()),
        expressions: vec!["coalesce(node_id, '00000000-0000-0000-0000-000000000000'::UUID)".to_string()],
    };

    // Remote has lowercase type cast (PostgreSQL normalizes to lowercase)
    let remote_idx = IndexInfo {
        index_name: "role_bindings_member_unique_idx".to_string(),
        columns: vec![],
        is_unique: true,
        is_primary: false,
        owning_constraint: None,
        index_method: "btree".to_string(),
        where_clause: Some("(principal_member_id IS NOT NULL)".to_string()),
        expressions: vec!["COALESCE(node_id, '00000000-0000-0000-0000-000000000000'::uuid)".to_string()],
    };

    assert!(
        !tables::indexes_differ(&local_idx, &remote_idx),
        "Expression-only indexes with equivalent coalesce expressions should not differ"
    );
}

#[test]
fn test_expression_index_type_cast_normalization() {
    // Verify type casts like ::UUID vs ::uuid don't cause false diffs
    let local_idx = IndexInfo {
        index_name: "idx_test".to_string(),
        columns: vec![],
        is_unique: false,
        is_primary: false,
        owning_constraint: None,
        index_method: "btree".to_string(),
        where_clause: None,
        expressions: vec!["coalesce(col, 'default'::TEXT)".to_string()],
    };

    let remote_idx = IndexInfo {
        index_name: "idx_test".to_string(),
        columns: vec![],
        is_unique: false,
        is_primary: false,
        owning_constraint: None,
        index_method: "btree".to_string(),
        where_clause: None,
        expressions: vec!["COALESCE(col, 'default'::text)".to_string()],
    };

    assert!(
        !tables::indexes_differ(&local_idx, &remote_idx),
        "Type cast differences should be normalized away"
    );
}

#[test]
fn test_expression_only_index_realistic_pipeline() {
    // Simulate EXACTLY what each side produces:
    // LOCAL: sqlparser's to_string() on coalesce expression
    // REMOTE: extract_index_expressions on pg_get_indexdef output + parse_pg_array on null columns

    // Remote side: what introspection returns after extract_index_expressions + parse_pg_array(null)
    // pg_get_indexdef returns: COALESCE(node_id, '00000000-...'::uuid)
    // extract_index_expressions extracts the expression part
    // parse_pg_array on null returns []
    let remote_idx = IndexInfo {
        index_name: "role_bindings_member_unique_idx".to_string(),
        columns: vec![],
        is_unique: true,
        is_primary: false,
        owning_constraint: None,
        index_method: "btree".to_string(),
        where_clause: Some("(principal_member_id IS NOT NULL)".to_string()),
        expressions: vec!["COALESCE(node_id, '00000000-0000-0000-0000-000000000000'::uuid)".to_string()],
    };

    // Local side: sqlparser parses CREATE INDEX ... (coalesce(...))
    // sqlparser's to_string() for coalesce typically produces: COALESCE(node_id, '00000000-...'::UUID)
    let local_idx = IndexInfo {
        index_name: "role_bindings_member_unique_idx".to_string(),
        columns: vec![],
        is_unique: true,
        is_primary: false,
        owning_constraint: None,
        index_method: "btree".to_string(),
        where_clause: Some("principal_member_id IS NOT NULL".to_string()),
        expressions: vec!["COALESCE(node_id, '00000000-0000-0000-0000-000000000000'::UUID)".to_string()],
    };

    eprintln!("=== REALISTIC PIPELINE TEST ===");
    eprintln!("Remote columns: {:?}", remote_idx.columns);
    eprintln!("Remote expressions: {:?}", remote_idx.expressions);
    eprintln!("Remote where: {:?}", remote_idx.where_clause);
    eprintln!("Local columns: {:?}", local_idx.columns);
    eprintln!("Local expressions: {:?}", local_idx.expressions);
    eprintln!("Local where: {:?}", local_idx.where_clause);

    assert!(
        !tables::indexes_differ(&local_idx, &remote_idx),
        "Realistic expression-only index should not produce a diff"
    );
}

#[test]
fn test_expression_only_index_end_to_end_parsing() {
    // Test the ACTUAL parser output vs the ACTUAL introspection output
    // Uses the real schema definition with mixed columns + expression
    use crate::parsing;

    let local_sql = r#"
CREATE TABLE "authz"."role_bindings" (
    "id" UUID NOT NULL DEFAULT gen_random_uuid(),
    "organization_id" UUID NOT NULL,
    "role_id" UUID NOT NULL,
    "scope" TEXT NOT NULL,
    "node_id" UUID,
    "principal_member_id" UUID
);
CREATE UNIQUE INDEX "role_bindings_member_unique_idx" ON "authz"."role_bindings" (organization_id, role_id, scope, coalesce(node_id, '00000000-0000-0000-0000-000000000000'::UUID), principal_member_id) WHERE principal_member_id IS NOT NULL;
"#;

    let files = vec![("schema.sql".to_string(), local_sql.to_string())];
    let local_schema = parsing::parse_schema_sql(&files).unwrap();
    let local_table = local_schema.tables.get("\"authz\".\"role_bindings\"")
        .expect("Table should exist");

    assert!(!local_table.indexes.is_empty(), "Should have at least one index");
    let local_idx = &local_table.indexes[0];

    eprintln!("=== LOCAL PARSER OUTPUT ===");
    eprintln!("index_name: {:?}", local_idx.index_name);
    eprintln!("columns: {:?}", local_idx.columns);
    eprintln!("expressions: {:?}", local_idx.expressions);
    eprintln!("where_clause: {:?}", local_idx.where_clause);
    eprintln!("is_unique: {}", local_idx.is_unique);
    eprintln!("index_method: {}", local_idx.index_method);

    // Simulate remote from pg_get_indexdef + introspection query
    // Remote has same regular columns + expression extracted from pg_get_indexdef
    let remote_idx = IndexInfo {
        index_name: "role_bindings_member_unique_idx".to_string(),
        columns: vec!["organization_id".to_string(), "role_id".to_string(), "scope".to_string(), "principal_member_id".to_string()],
        is_unique: true,
        is_primary: false,
        owning_constraint: None,
        index_method: "btree".to_string(),
        where_clause: Some("(principal_member_id IS NOT NULL)".to_string()),
        expressions: vec!["COALESCE(node_id, '00000000-0000-0000-0000-000000000000'::uuid)".to_string()],
    };

    eprintln!("=== REMOTE (simulated) ===");
    eprintln!("columns: {:?}", remote_idx.columns);
    eprintln!("expressions: {:?}", remote_idx.expressions);
    eprintln!("where_clause: {:?}", remote_idx.where_clause);

    let differs = tables::indexes_differ(local_idx, &remote_idx);
    eprintln!("indexes_differ result: {}", differs);

    assert!(
        !differs,
        "End-to-end: parsed local index should match introspected remote index"
    );
}

#[test]
fn test_generated_column_uuid_cast_normalization() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    let mut remote_table = TableInfo {
        schema: "authz".into(),
        table_name: "role_bindings".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    
    // Remote has implicit ::uuid cast from Postgres
    remote_table.columns.insert("scope_uuid".into(), ColumnInfo {
        column_name: "scope_uuid".into(),
        data_type: "uuid".into(),
        is_nullable: true,
        column_default: None,
        udt_name: "uuid".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: true,
        generation_expression: Some("CASE WHEN scope_type = 'file_node'::text AND scope_id IS NOT NULL THEN scope_id::uuid ELSE NULL::uuid END".into()),
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });
    
    // Local definition matches but without ::uuid cast on NULL
    let mut local_table = remote_table.clone();
    local_table.columns.insert("scope_uuid".into(), ColumnInfo {
        column_name: "scope_uuid".into(),
        data_type: "uuid".into(),
        is_nullable: true,
        column_default: None,
        udt_name: "uuid".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: true,
        generation_expression: Some("CASE WHEN scope_type = 'file_node' AND scope_id IS NOT NULL THEN scope_id::UUID ELSE NULL END".into()),
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    remote.tables.insert("role_bindings".into(), remote_table);
    local.tables.insert("role_bindings".into(), local_table);

    let diff = compute_diff(&remote, &local);
    
    // Should be no changes because normalization strips ::uuid
    if let Some(table_diff) = diff.table_changes.get("role_bindings") {
        assert!(table_diff.columns_to_modify.is_empty(), "Generated column diff should be empty");
        assert!(table_diff.columns_to_add.is_empty(), "Should not add column");
        assert!(table_diff.columns_to_drop.is_empty(), "Should not drop column");
    }
}

#[test]
fn test_trigger_function_schema_comparison() {
    // Local: Function without schema (implies public or search_path)
    // We normalize this to public.func if no schema is present
    let local = TriggerInfo {
        name: "trig_test".to_string(),
        events: vec!["UPDATE".to_string()],
        timing: "AFTER".to_string(),
        orientation: "ROW".to_string(),
        function_name: "my_func".to_string(), // No schema
        when_clause: None,
    };

    // Remote: Function with explicit public schema (introspection results typically have this)
    let remote = TriggerInfo {
        name: "trig_test".to_string(),
        events: vec!["UPDATE".to_string()],
        timing: "AFTER".to_string(),
        orientation: "ROW".to_string(),
        function_name: "public.my_func".to_string(), // Explicit schema
        when_clause: None,
    };

    assert!(!tables::triggers_differ(&local, &remote), "Trigger with implied public schema should match explicit public schema");

    // Local: Function with explicit custom schema
    let local_custom = TriggerInfo {
        name: "trig_test".to_string(),
        events: vec!["UPDATE".to_string()],
        timing: "AFTER".to_string(),
        orientation: "ROW".to_string(),
        function_name: "auth.my_func".to_string(),
        when_clause: None,
    };

    // Remote: Function with explicit custom schema
    let remote_custom = TriggerInfo {
        name: "trig_test".to_string(),
        events: vec!["UPDATE".to_string()],
        timing: "AFTER".to_string(),
        orientation: "ROW".to_string(),
        function_name: "auth.my_func".to_string(),
        when_clause: None,
    };

    assert!(!tables::triggers_differ(&local_custom, &remote_custom), "Trigger with matching custom schema should match");
}

#[test]
fn test_generated_column_custom_type_cast() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    // Remote has explicit cast to a custom type in another schema
    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "items".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    
    remote_table.columns.insert("status".into(), ColumnInfo {
        column_name: "status".into(),
        data_type: "text".into(), // Base type might effectively be text/enum
        is_nullable: true,
        column_default: None,
        udt_name: "text".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: true,
        // The introspection might return this with a cast to the custom enum type
        generation_expression: Some("('params'::text)::extensions.my_enum".into()),
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });
    
    let mut local_table = remote_table.clone();
    local_table.columns.insert("status".into(), ColumnInfo {
        column_name: "status".into(),
        data_type: "text".into(),
        is_nullable: true,
        column_default: None,
        udt_name: "text".into(),
        is_primary_key: false,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: true,
        // Local definition usually doesn't have the cast to custom type if user didn't write it, 
        // or just 'params'
        generation_expression: Some("'params'".into()),
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });

    remote.tables.insert("items".into(), remote_table);
    local.tables.insert("items".into(), local_table);

    let diff = compute_diff(&remote, &local);
    // Should NOT have any changes for "items" table
    assert!(diff.table_changes.is_empty(), "Generated column should not diff when ignoring custom type casts");
}

#[test]
fn test_ignore_unnamed_arg_diff() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    // Remote function with UNNAMED args (simulating introspection result where names are missing)
    // and explicitly NOT owned by extension (so our previous fix doesn't apply)
    remote.functions.insert(
        "\"public\".\"pgmq_read\"(text, integer, integer)".to_string(),
        FunctionInfo {
            schema: "public".into(),
            name: "pgmq_read".into(),
            args: vec![
                FunctionArg { name: "".into(), type_: "text".into(), mode: None, default_value: None },
                FunctionArg { name: "".into(), type_: "integer".into(), mode: None, default_value: None },
                FunctionArg { name: "".into(), type_: "integer".into(), mode: None, default_value: None },
            ],
            return_type: "table(msg_id bigint, read_ct integer, enqueued_at timestamp with time zone, vt timestamp with time zone, message jsonb)".into(),
            language: "sql".into(),
            definition: "SELECT ...".into(),
            volatility: None,
            is_strict: false,
            security_definer: true,
            config_params: vec![],
            grants: vec![],
            extension: None, // NOT extension owned
        }
    );

    // Local function definition (user declaring it, perhaps)
    // Args are named here, and uses type aliases
    local.functions.insert(
        "\"public\".\"pgmq_read\"(text, integer, integer)".to_string(),
        FunctionInfo {
            schema: "public".into(),
            name: "pgmq_read".into(),
            args: vec![
                FunctionArg { name: "queue_name".into(), type_: "text".into(), mode: None, default_value: None },
                FunctionArg { name: "vt".into(), type_: "int".into(), mode: None, default_value: None },
                FunctionArg { name: "qty".into(), type_: "int".into(), mode: None, default_value: None },
            ],
            return_type: "table(msg_id bigint, read_ct int, enqueued_at timestamptz, vt timestamptz, message jsonb)".into(),
            language: "sql".into(),
            definition: "SELECT ...".into(),
            volatility: None,
            is_strict: false,
            security_definer: true,
            config_params: vec![],
            grants: vec![],
            extension: None, // User definition doesn't know about extension ownership
        }
    );

    let diff = compute_diff(&remote, &local);
    
    // DESIRED BEHAVIOR: Ignore difference because remote is extension-owned
    assert!(diff.functions_to_drop.is_empty(), "Should not drop extension function");
    assert!(diff.functions_to_create.is_empty(), "Should not recreate extension function");
}


#[test]
fn test_ignore_bigserial_diff() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    // Remote: Introspected as bigint with nextval default
    let mut remote_table = TableInfo {
        schema: "public".into(),
        table_name: "backfill_jobs".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    remote_table.columns.insert("id".into(), ColumnInfo {
        column_name: "id".into(),
        data_type: "bigint".into(), // Normalized from int8
        is_nullable: false,
        column_default: Some("nextval('backfill_jobs_id_seq'::regclass)".into()),
        udt_name: "int8".into(),
        is_primary_key: true,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });
    remote.tables.insert("backfill_jobs".into(), remote_table);

    // Local: Parsed as BIGSERIAL without default
    let mut local_table = TableInfo {
        schema: "public".into(),
        table_name: "backfill_jobs".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    local_table.columns.insert("id".into(), ColumnInfo {
        column_name: "id".into(),
        data_type: "BIGSERIAL".into(), // As parsed
        is_nullable: false,
        column_default: None, // No explicit default
        udt_name: "int8".into(),
        is_primary_key: true,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });
    local.tables.insert("backfill_jobs".into(), local_table);

    let diff = compute_diff(&remote, &local);
    
    // Check if table exists in changes
    if let Some(table_diff) = diff.table_changes.get("backfill_jobs") {
        // If it exists, ensure no columns are modified
        assert!(table_diff.columns_to_modify.is_empty(), "Should not modify BIGSERIAL column: {:?}", table_diff.columns_to_modify);
    }
}

#[test]
fn test_ignore_implicit_sequence_drop() {
    let mut remote = DbSchema::new();
    let mut local = DbSchema::new();

    // Remote has a sequence owned by a table column
    let seq_name = "backfill_jobs_id_seq".to_string();
    remote.sequences.insert(seq_name.clone(), SequenceInfo {
        schema: "public".into(),
        name: seq_name.clone(),
        data_type: "bigint".into(),
        start_value: 1,
        min_value: 1,
        max_value: 9223372036854775807,
        increment: 1,
        cycle: false,
        cache_size: 1,
        owned_by: Some("public.backfill_jobs.id".into()), // Owned by table column
        extension: None,
        comment: None,
    });

    // Local has the table and column (implicitly owning the sequence via BIGSERIAL), but NOT the sequence object itself
    let mut local_table = TableInfo {
        schema: "public".into(),
        table_name: "backfill_jobs".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![], extension: None,
        comment: None,
    };
    local_table.columns.insert("id".into(), ColumnInfo {
        column_name: "id".into(),
        data_type: "BIGSERIAL".into(),
        is_nullable: false,
        column_default: None,
        udt_name: "BIGSERIAL".into(),
        is_primary_key: true,
        is_unique: false,
        is_identity: false,
        identity_generation: None,
        is_generated: false,
        generation_expression: None,
        collation: None,
        enum_name: None,
        is_array: false,
        comment: None,
    });
    local.tables.insert("backfill_jobs".into(), local_table);

    // Compute diff
    let diff = compute_diff(&remote, &local);

    // Sequence should NOT be dropped because it is owned by a local table column
    assert!(!diff.sequences_to_drop.contains(&seq_name), "Should not drop explicitly owned sequence");
}
