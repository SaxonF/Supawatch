use super::*;
use super::roles::generate_create_extension;
use crate::diff::*;
use crate::schema::*;
use std::collections::HashMap;
use super::constraints::{generate_create_index, generate_create_trigger, generate_add_foreign_key};
use super::objects::{generate_create_sequence, generate_create_view};
use super::types::{generate_create_domain, generate_create_composite_type};
use super::tables::generate_alter_table;

#[test]
fn test_generate_sql_full() {
    // Setup diff
    let diff = SchemaDiff {
        tables_to_create: vec![],
        tables_to_drop: vec![],
        table_changes: HashMap::new(),
        enum_changes: vec![],
        functions_to_create: vec![FunctionInfo {
            schema: "public".to_string(),
            name: "new_func".to_string(),
            args: vec![FunctionArg {
                name: "a".to_string(),
                type_: "int".to_string(),
                mode: None,
                default_value: None,
            }],
            return_type: "void".to_string(),
            language: "plpgsql".to_string(),
            definition: "BEGIN END;".to_string(),
            volatility: None,
            is_strict: false,
            security_definer: false,
        }],
        functions_to_drop: vec!["old_func".to_string()],
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

    // Run generator
    let schema = DbSchema::new();
    let sql = generate_sql(&diff, &schema);

    assert!(sql.contains("CREATE OR REPLACE FUNCTION \"public\".\"new_func\""));
    assert!(sql.contains("DROP FUNCTION IF EXISTS \"old_func\" CASCADE"));
}

#[test]
fn test_generate_create_index_with_method_and_where() {
    let idx = IndexInfo {
        index_name: "idx_active_users".to_string(),
        columns: vec!["email".to_string()],
        is_unique: true,
        is_primary: false,
        owning_constraint: None,
        index_method: "gin".to_string(),
        where_clause: Some("active = true".to_string()),
        expressions: vec![],
    };

    let sql = generate_create_index("\"public\".\"users\"", &idx);
    assert!(sql.contains("USING gin"));
    assert!(sql.contains("WHERE active = true"));
}

#[test]
fn test_generate_trigger_with_when() {
    let trigger = TriggerInfo {
        name: "notify_changes".to_string(),
        events: vec!["UPDATE".to_string()],
        timing: "AFTER".to_string(),
        orientation: "ROW".to_string(),
        function_name: "notify_trigger".to_string(),
        when_clause: Some("OLD.status IS DISTINCT FROM NEW.status".to_string()),
    };

    let sql = generate_create_trigger("\"public\".\"users\"", &trigger);
    assert!(sql.contains("WHEN (OLD.status IS DISTINCT FROM NEW.status)"));
}

#[test]
fn test_generate_foreign_key_with_on_update() {
    let fk = ForeignKeyInfo {
        constraint_name: "fk_user_org".to_string(),
        column_name: "org_id".to_string(),
        foreign_table: "organizations".to_string(),
        foreign_column: "id".to_string(),
        on_delete: "CASCADE".to_string(),
        on_update: "SET NULL".to_string(),
    };

    let sql = generate_add_foreign_key("\"public\".\"users\"", &fk);
    assert!(sql.contains("ON DELETE CASCADE"));
    assert!(sql.contains("ON UPDATE SET NULL"));
}

#[test]
fn test_generate_create_sequence() {
    let seq = SequenceInfo {
        schema: "public".to_string(),
        name: "user_id_seq".to_string(),
        data_type: "bigint".to_string(),
        start_value: 1,
        min_value: 1,
        max_value: 1000000,
        increment: 1,
        cycle: false,
        cache_size: 10,
        owned_by: Some("users.id".to_string()),
        comment: None,
    };

    let sql = generate_create_sequence(&seq);
    assert!(sql.contains("CREATE SEQUENCE \"public\".\"user_id_seq\""));
    assert!(sql.contains("CACHE 10"));
    assert!(sql.contains("OWNED BY users.id"));
}

#[test]
fn test_generate_create_view() {
    let view = ViewInfo {
        schema: "public".to_string(),
        name: "active_users".to_string(),
        definition: "SELECT * FROM users WHERE active = true".to_string(),
        is_materialized: false,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec!["security_barrier=true".to_string()],
        check_option: None,
    };

    let sql = generate_create_view(&view);
    assert!(sql.contains("CREATE OR REPLACE VIEW \"public\".\"active_users\""));
    assert!(sql.contains("WITH (security_barrier=true)"));
}

#[test]
fn test_generate_materialized_view() {
    let view = ViewInfo {
        schema: "public".to_string(),
        name: "user_stats".to_string(),
        definition: "SELECT user_id, count(*) FROM posts GROUP BY user_id".to_string(),
        is_materialized: true,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: None,
    };

    let sql = generate_create_view(&view);
    assert!(sql.contains("CREATE MATERIALIZED VIEW \"public\".\"user_stats\""));
}

#[test]
fn test_generate_create_domain() {
    let domain = DomainInfo {
        schema: "public".to_string(),
        name: "email_address".to_string(),
        base_type: "text".to_string(),
        default_value: None,
        is_not_null: true,
        check_constraints: vec![DomainCheckConstraint {
            name: Some("valid_email".to_string()),
            expression: "CHECK (VALUE ~ '^[^@]+@[^@]+$')".to_string(),
        }],
        collation: None,
        comment: None,
    };

    let sql = generate_create_domain(&domain);
    assert!(sql.contains("CREATE DOMAIN \"public\".\"email_address\""));
    assert!(sql.contains("NOT NULL"));
    assert!(sql.contains("CONSTRAINT \"valid_email\""));
}

#[test]
fn test_generate_composite_type() {
    let comp_type = CompositeTypeInfo {
        schema: "public".to_string(),
        name: "address".to_string(),
        attributes: vec![
            CompositeTypeAttribute {
                name: "street".to_string(),
                data_type: "text".to_string(),
                collation: None,
            },
            CompositeTypeAttribute {
                name: "city".to_string(),
                data_type: "text".to_string(),
                collation: None,
            },
        ],
        comment: None,
    };

    let sql = generate_create_composite_type(&comp_type);
    assert!(sql.contains("CREATE TYPE \"public\".\"address\" AS"));
    assert!(sql.contains("\"street\" text"));
    assert!(sql.contains("\"city\" text"));
}

#[test]
fn test_generate_extension() {
    let ext = ExtensionInfo {
        name: "uuid-ossp".to_string(),
        version: Some("1.1".to_string()),
        schema: Some("extensions".to_string()),
    };

    let sql = generate_create_extension(&ext);
    assert!(sql.contains("CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\""));
    assert!(sql.contains("WITH SCHEMA \"extensions\""));
    assert!(sql.contains("VERSION '1.1'"));
}

#[test]
fn test_drop_type_quoting() {
    let diff = SchemaDiff {
        tables_to_create: vec![],
        tables_to_drop: vec![],
        table_changes: HashMap::new(),
        enum_changes: vec![EnumChange {
            name: "\"public\".\"status\"".to_string(), // Already quoted/qualified
            type_: EnumChangeType::Drop,
            values_to_add: None,
        }],
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
        composite_types_to_drop: vec!["\"public\".\"addr\"".to_string()], // Already quoted
        domains_to_create: vec![],
        domains_to_drop: vec![],
        roles_to_create: vec![],
        roles_to_drop: vec![],
        roles_to_update: vec![],
    };

    let schema = DbSchema::new();
    let sql = generate_sql(&diff, &schema);

    // Should NOT be ""public"."status""
    assert!(sql.contains("DROP TYPE IF EXISTS \"public\".\"status\" CASCADE;"));
    assert!(sql.contains("DROP TYPE IF EXISTS \"public\".\"addr\" CASCADE;"));
}

#[test]
fn test_generate_alter_table_columns() {
    let table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::from([
            ("age".into(), ColumnInfo {
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
            })
        ]),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![],
        comment: None,
    };

    let mut table_diff = TableDiff {
        columns_to_add: vec!["email".into()],
        columns_to_drop: vec!["old_col".into()],
        columns_to_modify: vec![
            ColumnModification {
                column_name: "age".into(),
                changes: ColumnChangeDetail {
                    type_change: Some(("integer".into(), "bigint".into())),
                    nullable_change: Some((true, false)),
                    default_change: Some((None, Some("18".into()))),
                    identity_change: None,
                    collation_change: None,
                    comment_change: None,
                },
            }
        ],
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

    // We need to mock the full column info for "email" so it can be added
    let mut local_table = table.clone();
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

    let statements = generate_alter_table("\"public\".\"users\"", &table_diff, &local_table);
    
    // Add column
    assert!(statements.iter().any(|s| s.contains("ADD COLUMN \"email\" text NOT NULL")));
    // Drop column
    assert!(statements.iter().any(|s| s.contains("DROP COLUMN IF EXISTS \"old_col\"")));
    // Modify column type
    assert!(statements.iter().any(|s| s.contains("ALTER COLUMN \"age\" TYPE bigint USING \"age\"::bigint")));
    // Modify column nullability
    assert!(statements.iter().any(|s| s.contains("ALTER COLUMN \"age\" SET NOT NULL")));
    // Modify column default
    assert!(statements.iter().any(|s| s.contains("ALTER COLUMN \"age\" SET DEFAULT 18")));
}
