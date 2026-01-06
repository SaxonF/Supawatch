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
