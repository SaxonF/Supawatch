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
        functions_to_drop: vec!["\"old_func\"".to_string()],
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
        foreign_schema: "public".to_string(),
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

// ============================================================================
// Additional Generator Tests for Full Postgres Feature Coverage
// ============================================================================

#[test]
fn test_generate_create_role_with_all_options() {
    use super::roles::generate_create_role;

    let role = RoleInfo {
        name: "app_admin".to_string(),
        superuser: true,
        create_db: true,
        create_role: true,
        inherit: true,
        login: true,
        replication: true,
        bypass_rls: true,
        connection_limit: 10,
        valid_until: Some("2025-12-31".to_string()),
        password: Some("secret".to_string()),
    };

    let sql = generate_create_role(&role);
    assert!(sql.contains("CREATE ROLE \"app_admin\""));
    assert!(sql.contains("SUPERUSER"));
    assert!(sql.contains("CREATEDB"));
    assert!(sql.contains("CREATEROLE"));
    assert!(sql.contains("LOGIN"));
    assert!(sql.contains("REPLICATION"));
    assert!(sql.contains("BYPASSRLS"));
    assert!(sql.contains("CONNECTION LIMIT 10"));
    assert!(sql.contains("VALID UNTIL '2025-12-31'"));
    assert!(sql.contains("PASSWORD 'secret'"));
}

#[test]
fn test_generate_alter_role() {
    use super::roles::generate_alter_role;

    let role = RoleInfo {
        name: "app_user".to_string(),
        superuser: false,
        create_db: true,
        create_role: false,
        inherit: true,
        login: true,
        replication: false,
        bypass_rls: false,
        connection_limit: -1,
        valid_until: None,
        password: None,
    };

    let sql = generate_alter_role(&role);
    assert!(sql.contains("ALTER ROLE \"app_user\""));
    assert!(sql.contains("NOSUPERUSER"));
    assert!(sql.contains("CREATEDB"));
    assert!(sql.contains("LOGIN"));
}

#[test]
fn test_generate_create_enum() {
    use super::types::generate_create_enum;

    let sql = generate_create_enum("\"public\".\"status\"", &vec![
        "pending".to_string(),
        "active".to_string(),
        "cancelled".to_string(),
    ]);

    assert!(sql.contains("CREATE TYPE \"public\".\"status\" AS ENUM"));
    assert!(sql.contains("'pending'"));
    assert!(sql.contains("'active'"));
    assert!(sql.contains("'cancelled'"));
}

#[test]
fn test_generate_function_with_volatility() {
    use super::objects::generate_create_function;

    let func = FunctionInfo {
        schema: "public".to_string(),
        name: "get_config".to_string(),
        args: vec![],
        return_type: "text".to_string(),
        language: "sql".to_string(),
        definition: "SELECT 'value'".to_string(),
        volatility: Some("STABLE".to_string()),
        is_strict: false,
        security_definer: false,
    };

    let sql = generate_create_function(&func);
    assert!(sql.contains("STABLE"));
}

#[test]
fn test_generate_function_with_strict() {
    use super::objects::generate_create_function;

    let func = FunctionInfo {
        schema: "public".to_string(),
        name: "add_numbers".to_string(),
        args: vec![
            FunctionArg { name: "a".to_string(), type_: "integer".to_string(), mode: None, default_value: None },
            FunctionArg { name: "b".to_string(), type_: "integer".to_string(), mode: None, default_value: None },
        ],
        return_type: "integer".to_string(),
        language: "sql".to_string(),
        definition: "SELECT a + b".to_string(),
        volatility: Some("IMMUTABLE".to_string()),
        is_strict: true,
        security_definer: false,
    };

    let sql = generate_create_function(&func);
    assert!(sql.contains("IMMUTABLE"));
    assert!(sql.contains("STRICT"));
}

#[test]
fn test_generate_function_with_security_definer() {
    use super::objects::generate_create_function;

    let func = FunctionInfo {
        schema: "public".to_string(),
        name: "get_user_id".to_string(),
        args: vec![],
        return_type: "uuid".to_string(),
        language: "sql".to_string(),
        definition: "SELECT auth.uid()".to_string(),
        volatility: None,
        is_strict: false,
        security_definer: true,
    };

    let sql = generate_create_function(&func);
    assert!(sql.contains("SECURITY DEFINER"));
}

#[test]
fn test_generate_function_with_default_args() {
    use super::objects::generate_create_function;

    let func = FunctionInfo {
        schema: "public".to_string(),
        name: "greet".to_string(),
        args: vec![
            FunctionArg { name: "name".to_string(), type_: "text".to_string(), mode: None, default_value: Some("'World'".to_string()) },
        ],
        return_type: "text".to_string(),
        language: "sql".to_string(),
        definition: "SELECT 'Hello, ' || name".to_string(),
        volatility: None,
        is_strict: false,
        security_definer: false,
    };

    let sql = generate_create_function(&func);
    assert!(sql.contains("DEFAULT 'World'"));
}

#[test]
fn test_generate_alter_sequence() {
    use super::objects::generate_alter_sequence;

    let seq = SequenceInfo {
        schema: "public".to_string(),
        name: "order_seq".to_string(),
        data_type: "bigint".to_string(),
        start_value: 1,
        min_value: 1,
        max_value: 9999999,
        increment: 5,
        cycle: true,
        cache_size: 20,
        owned_by: None,
        comment: None,
    };

    let sql = generate_alter_sequence(&seq);
    assert!(sql.contains("ALTER SEQUENCE \"public\".\"order_seq\""));
    assert!(sql.contains("INCREMENT BY 5"));
    assert!(sql.contains("CACHE 20"));
    assert!(sql.contains("CYCLE"));
}

#[test]
fn test_generate_identity_column_change() {
    let table = TableInfo {
        schema: "public".into(),
        table_name: "items".into(),
        columns: HashMap::from([
            ("id".into(), ColumnInfo {
                column_name: "id".into(),
                data_type: "integer".into(),
                is_nullable: false,
                column_default: None,
                udt_name: "int4".into(),
                is_primary_key: true,
                is_unique: true,
                is_identity: true,
                identity_generation: Some("ALWAYS".to_string()),
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

    let table_diff = TableDiff {
        columns_to_add: vec![],
        columns_to_drop: vec![],
        columns_to_modify: vec![
            ColumnModification {
                column_name: "id".into(),
                changes: ColumnChangeDetail {
                    type_change: None,
                    nullable_change: None,
                    default_change: None,
                    identity_change: Some((None, Some("ALWAYS".to_string()))),
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

    let statements = generate_alter_table("\"public\".\"items\"", &table_diff, &table);
    assert!(statements.iter().any(|s| s.contains("ADD GENERATED ALWAYS AS IDENTITY")));
}

#[test]
fn test_generate_collation_change() {
    let table = TableInfo {
        schema: "public".into(),
        table_name: "data".into(),
        columns: HashMap::from([
            ("name".into(), ColumnInfo {
                column_name: "name".into(),
                data_type: "text".into(),
                is_nullable: true,
                column_default: None,
                udt_name: "text".into(),
                is_primary_key: false,
                is_unique: false,
                is_identity: false,
                identity_generation: None,
                collation: Some("C".to_string()),
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

    let table_diff = TableDiff {
        columns_to_add: vec![],
        columns_to_drop: vec![],
        columns_to_modify: vec![
            ColumnModification {
                column_name: "name".into(),
                changes: ColumnChangeDetail {
                    type_change: None,
                    nullable_change: None,
                    default_change: None,
                    identity_change: None,
                    collation_change: Some((None, Some("C".to_string()))),
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

    let statements = generate_alter_table("\"public\".\"data\"", &table_diff, &table);
    assert!(statements.iter().any(|s| s.contains("COLLATE")));
}

#[test]
fn test_generate_check_constraint_add() {
    let table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![
            CheckConstraintInfo {
                name: "valid_age".into(),
                expression: "CHECK (age >= 0 AND age < 200)".into(),
                columns: vec![],
            }
        ],
        comment: None,
    };

    let table_diff = TableDiff {
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
        check_constraints_to_create: vec![
            CheckConstraintInfo {
                name: "valid_age".into(),
                expression: "CHECK (age >= 0 AND age < 200)".into(),
                columns: vec![],
            }
        ],
        check_constraints_to_drop: vec![],
        foreign_keys_to_create: vec![],
        foreign_keys_to_drop: vec![],
        comment_change: None,
    };

    let statements = generate_alter_table("\"public\".\"users\"", &table_diff, &table);
    assert!(statements.iter().any(|s| s.contains("ADD CONSTRAINT \"valid_age\"")));
}

#[test]
fn test_generate_rls_enable() {
    let table = TableInfo {
        schema: "public".into(),
        table_name: "posts".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: true,
        policies: vec![],
        check_constraints: vec![],
        comment: None,
    };

    let table_diff = TableDiff {
        columns_to_add: vec![],
        columns_to_drop: vec![],
        columns_to_modify: vec![],
        rls_change: Some(true),
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

    let statements = generate_alter_table("\"public\".\"posts\"", &table_diff, &table);
    assert!(statements.iter().any(|s| s.contains("ENABLE ROW LEVEL SECURITY")));
}

#[test]
fn test_generate_rls_disable() {
    let table = TableInfo {
        schema: "public".into(),
        table_name: "posts".into(),
        columns: HashMap::new(),
        foreign_keys: vec![],
        indexes: vec![],
        triggers: vec![],
        rls_enabled: false,
        policies: vec![],
        check_constraints: vec![],
        comment: None,
    };

    let table_diff = TableDiff {
        columns_to_add: vec![],
        columns_to_drop: vec![],
        columns_to_modify: vec![],
        rls_change: Some(false),
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

    let statements = generate_alter_table("\"public\".\"posts\"", &table_diff, &table);
    assert!(statements.iter().any(|s| s.contains("DISABLE ROW LEVEL SECURITY")));
}

#[test]
fn test_generate_policy_with_using_and_check() {
    use super::constraints::generate_create_policy;

    let policy = PolicyInfo {
        name: "manage_own".to_string(),
        cmd: "ALL".to_string(),
        roles: vec!["authenticated".to_string()],
        qual: Some("user_id = auth.uid()".to_string()),
        with_check: Some("user_id = auth.uid()".to_string()),
    };

    let sql = generate_create_policy("\"public\".\"posts\"", &policy);
    assert!(sql.contains("CREATE POLICY \"manage_own\""));
    assert!(sql.contains("FOR ALL"));
    assert!(sql.contains("TO authenticated"));
    assert!(sql.contains("USING (user_id = auth.uid())"));
    assert!(sql.contains("WITH CHECK (user_id = auth.uid())"));
}

#[test]
fn test_generate_trigger_with_multiple_events() {
    let trigger = TriggerInfo {
        name: "audit_changes".to_string(),
        events: vec!["INSERT".to_string(), "UPDATE".to_string(), "DELETE".to_string()],
        timing: "AFTER".to_string(),
        orientation: "ROW".to_string(),
        function_name: "audit_trigger_func".to_string(),
        when_clause: None,
    };

    let sql = generate_create_trigger("\"public\".\"data\"", &trigger);
    assert!(sql.contains("INSERT OR UPDATE OR DELETE"));
    assert!(sql.contains("AFTER"));
    assert!(sql.contains("FOR EACH ROW"));
}

#[test]
fn test_generate_index_with_expression() {
    let idx = IndexInfo {
        index_name: "idx_lower_email".to_string(),
        columns: vec![],
        is_unique: true,
        is_primary: false,
        owning_constraint: None,
        index_method: "btree".to_string(),
        where_clause: None,
        expressions: vec!["lower(email)".to_string()],
    };

    let sql = generate_create_index("\"public\".\"users\"", &idx);
    assert!(sql.contains("CREATE UNIQUE INDEX"));
    assert!(sql.contains("(lower(email))"));
}

#[test]
fn test_generate_drop_table() {
    let diff = SchemaDiff {
        tables_to_create: vec![],
        tables_to_drop: vec!["\"public\".\"old_table\"".to_string()],
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

    let schema = DbSchema::new();
    let sql = generate_sql(&diff, &schema);
    assert!(sql.contains("DROP TABLE IF EXISTS \"public\".\"old_table\" CASCADE"));
}

#[test]
fn test_generate_drop_view() {
    let mut schema = DbSchema::new();
    schema.views.insert("\"public\".\"old_view\"".to_string(), ViewInfo {
        schema: "public".to_string(),
        name: "old_view".to_string(),
        definition: "SELECT 1".to_string(),
        is_materialized: false,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: None,
    });

    let diff = SchemaDiff {
        tables_to_create: vec![],
        tables_to_drop: vec![],
        table_changes: HashMap::new(),
        enum_changes: vec![],
        functions_to_create: vec![],
        functions_to_drop: vec![],
        functions_to_update: vec![],
        views_to_create: vec![],
        views_to_drop: vec!["\"public\".\"old_view\"".to_string()],
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

    let sql = generate_sql(&diff, &schema);
    assert!(sql.contains("DROP VIEW IF EXISTS"));
}

#[test]
fn test_generate_drop_materialized_view() {
    let mut schema = DbSchema::new();
    schema.views.insert("\"public\".\"cached_stats\"".to_string(), ViewInfo {
        schema: "public".to_string(),
        name: "cached_stats".to_string(),
        definition: "SELECT 1".to_string(),
        is_materialized: true,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: None,
    });

    let diff = SchemaDiff {
        tables_to_create: vec![],
        tables_to_drop: vec![],
        table_changes: HashMap::new(),
        enum_changes: vec![],
        functions_to_create: vec![],
        functions_to_drop: vec![],
        functions_to_update: vec![],
        views_to_create: vec![],
        views_to_drop: vec!["\"public\".\"cached_stats\"".to_string()],
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

    let sql = generate_sql(&diff, &schema);
    assert!(sql.contains("DROP MATERIALIZED VIEW IF EXISTS"));
}

#[test]
fn test_generate_drop_sequence() {
    let diff = SchemaDiff {
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
        sequences_to_drop: vec!["\"public\".\"old_seq\"".to_string()],
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

    let schema = DbSchema::new();
    let sql = generate_sql(&diff, &schema);
    assert!(sql.contains("DROP SEQUENCE IF EXISTS"));
}

#[test]
fn test_generate_drop_extension() {
    let diff = SchemaDiff {
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
        extensions_to_drop: vec!["postgis".to_string()],
        composite_types_to_create: vec![],
        composite_types_to_drop: vec![],
        domains_to_create: vec![],
        domains_to_drop: vec![],
        roles_to_create: vec![],
        roles_to_drop: vec![],
        roles_to_update: vec![],
    };

    let schema = DbSchema::new();
    let sql = generate_sql(&diff, &schema);
    assert!(sql.contains("DROP EXTENSION IF EXISTS \"postgis\" CASCADE"));
}

#[test]
fn test_generate_drop_role() {
    let diff = SchemaDiff {
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
        roles_to_drop: vec!["old_role".to_string()],
        roles_to_update: vec![],
    };

    let schema = DbSchema::new();
    let sql = generate_sql(&diff, &schema);
    assert!(sql.contains("DROP ROLE IF EXISTS \"old_role\""));
}

#[test]
fn test_generate_drop_domain() {
    let diff = SchemaDiff {
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
        domains_to_drop: vec!["\"public\".\"old_domain\"".to_string()],
        roles_to_create: vec![],
        roles_to_drop: vec![],
        roles_to_update: vec![],
    };

    let schema = DbSchema::new();
    let sql = generate_sql(&diff, &schema);
    assert!(sql.contains("DROP DOMAIN IF EXISTS"));
}

#[test]
fn test_generate_domain_with_collation() {
    let domain = DomainInfo {
        schema: "public".to_string(),
        name: "ci_text".to_string(),
        base_type: "text".to_string(),
        default_value: None,
        is_not_null: false,
        check_constraints: vec![],
        collation: Some("C".to_string()),
        comment: None,
    };

    let sql = generate_create_domain(&domain);
    assert!(sql.contains("COLLATE \"C\""));
}

#[test]
fn test_generate_domain_with_default() {
    let domain = DomainInfo {
        schema: "public".to_string(),
        name: "nonneg_int".to_string(),
        base_type: "integer".to_string(),
        default_value: Some("0".to_string()),
        is_not_null: false,
        check_constraints: vec![
            DomainCheckConstraint {
                name: Some("positive".to_string()),
                expression: "CHECK (VALUE >= 0)".to_string(),
            }
        ],
        collation: None,
        comment: None,
    };

    let sql = generate_create_domain(&domain);
    assert!(sql.contains("DEFAULT 0"));
    assert!(sql.contains("CONSTRAINT \"positive\""));
}

#[test]
fn test_generate_composite_type_with_collation() {
    let comp_type = CompositeTypeInfo {
        schema: "public".to_string(),
        name: "person_name".to_string(),
        attributes: vec![
            CompositeTypeAttribute {
                name: "first_name".to_string(),
                data_type: "text".to_string(),
                collation: Some("C".to_string()),
            },
            CompositeTypeAttribute {
                name: "last_name".to_string(),
                data_type: "text".to_string(),
                collation: None,
            },
        ],
        comment: None,
    };

    let sql = generate_create_composite_type(&comp_type);
    assert!(sql.contains("COLLATE \"C\""));
}

#[test]
fn test_generate_view_with_check_option() {
    let view = ViewInfo {
        schema: "public".to_string(),
        name: "active_users".to_string(),
        definition: "SELECT * FROM users WHERE active = true".to_string(),
        is_materialized: false,
        columns: vec![],
        indexes: vec![],
        comment: None,
        with_options: vec![],
        check_option: Some("LOCAL".to_string()),
    };

    let sql = generate_create_view(&view);
    assert!(sql.contains("WITH LOCAL CHECK OPTION"));
}

#[test]
fn test_generate_index_drop_with_constraint() {
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

    let table_diff = TableDiff {
        columns_to_add: vec![],
        columns_to_drop: vec![],
        columns_to_modify: vec![],
        rls_change: None,
        policies_to_create: vec![],
        policies_to_drop: vec![],
        triggers_to_create: vec![],
        triggers_to_drop: vec![],
        indexes_to_create: vec![],
        indexes_to_drop: vec![
            IndexInfo {
                index_name: "unique_email".into(),
                columns: vec!["email".into()],
                is_unique: true,
                is_primary: false,
                owning_constraint: Some("unique_email".into()), // Owned by constraint
                index_method: "btree".into(),
                where_clause: None,
                expressions: vec![],
            }
        ],
        check_constraints_to_create: vec![],
        check_constraints_to_drop: vec![],
        foreign_keys_to_create: vec![],
        foreign_keys_to_drop: vec![],
        comment_change: None,
    };

    let statements = generate_alter_table("\"public\".\"users\"", &table_diff, &table);
    // Should drop the constraint, not the index directly
    assert!(statements.iter().any(|s| s.contains("DROP CONSTRAINT IF EXISTS \"unique_email\"")));
}

#[test]
fn test_generate_unique_constraint_via_index() {
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

    let table_diff = TableDiff {
        columns_to_add: vec![],
        columns_to_drop: vec![],
        columns_to_modify: vec![],
        rls_change: None,
        policies_to_create: vec![],
        policies_to_drop: vec![],
        triggers_to_create: vec![],
        triggers_to_drop: vec![],
        indexes_to_create: vec![
            IndexInfo {
                index_name: "unique_email".into(),
                columns: vec!["email".into()],
                is_unique: true,
                is_primary: false,
                owning_constraint: Some("unique_email".into()), // Represents UNIQUE constraint
                index_method: "btree".into(),
                where_clause: None,
                expressions: vec![],
            }
        ],
        indexes_to_drop: vec![],
        check_constraints_to_create: vec![],
        check_constraints_to_drop: vec![],
        foreign_keys_to_create: vec![],
        foreign_keys_to_drop: vec![],
        comment_change: None,
    };

    let statements = generate_alter_table("\"public\".\"users\"", &table_diff, &table);
    assert!(statements.iter().any(|s| s.contains("ADD CONSTRAINT \"unique_email\" UNIQUE")));
}

#[test]
fn test_generate_drop_default() {
    let table = TableInfo {
        schema: "public".into(),
        table_name: "users".into(),
        columns: HashMap::from([
            ("age".into(), ColumnInfo {
                column_name: "age".into(),
                data_type: "integer".into(),
                is_nullable: true,
                column_default: None, // No default now
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

    let table_diff = TableDiff {
        columns_to_add: vec![],
        columns_to_drop: vec![],
        columns_to_modify: vec![
            ColumnModification {
                column_name: "age".into(),
                changes: ColumnChangeDetail {
                    type_change: None,
                    nullable_change: None,
                    default_change: Some((Some("18".into()), None)), // Dropping default
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

    let statements = generate_alter_table("\"public\".\"users\"", &table_diff, &table);
    assert!(statements.iter().any(|s| s.contains("DROP DEFAULT")));
}

#[test]
fn test_generate_drop_identity() {
    let table = TableInfo {
        schema: "public".into(),
        table_name: "items".into(),
        columns: HashMap::from([
            ("id".into(), ColumnInfo {
                column_name: "id".into(),
                data_type: "integer".into(),
                is_nullable: false,
                column_default: None,
                udt_name: "int4".into(),
                is_primary_key: true,
                is_unique: true,
                is_identity: false, // No longer identity
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

    let table_diff = TableDiff {
        columns_to_add: vec![],
        columns_to_drop: vec![],
        columns_to_modify: vec![
            ColumnModification {
                column_name: "id".into(),
                changes: ColumnChangeDetail {
                    type_change: None,
                    nullable_change: None,
                    default_change: None,
                    identity_change: Some((Some("ALWAYS".to_string()), None)), // Dropping identity
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

    let statements = generate_alter_table("\"public\".\"items\"", &table_diff, &table);
    assert!(statements.iter().any(|s| s.contains("DROP IDENTITY")));
}



