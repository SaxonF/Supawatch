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
        check_constraints: vec![],
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
        check_constraints: vec![],
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
        check_constraints: vec![],
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
        check_constraints: vec![],
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
        check_constraints: vec![],
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
        check_constraints: vec![],
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
        check_constraints: vec![],
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
        check_constraints: vec![],
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
        check_constraints: vec![],
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
        values: vec!["active".into(), "inactive".into()],
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
        values: vec!["active".into(), "inactive".into()],
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
            values: vec!["active".to_string(), "inactive".to_string()],
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
        check_constraints: vec![],
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
        check_constraints: vec![],
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
        check_constraints: vec![],
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
            values: vec!["active".to_string(), "inactive".to_string()],
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
        check_constraints: vec![],
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
            values: vec!["active".to_string(), "inactive".to_string()],
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
        check_constraints: vec![],
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

