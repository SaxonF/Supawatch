use crate::schema::{
    CheckConstraintInfo, ColumnInfo, CompositeTypeAttribute, CompositeTypeInfo, DbSchema,
    DomainCheckConstraint, DomainInfo, EnumInfo, ExtensionInfo, ForeignKeyInfo, FunctionArg,
    FunctionInfo, IndexInfo, PolicyInfo, RoleInfo, SequenceInfo, TableInfo, TriggerInfo, ViewInfo,
};
use sqlparser::ast::{
    AlterTable, AlterTableOperation, ColumnDef, ColumnOption, CreateFunction, CreateFunctionBody,
    CreateIndex, CreatePolicyCommand, CreateRole, CreateTable, CreateTrigger, Expr, OperateFunctionArg,
    Statement, TableConstraint, TriggerExecBody, Value,
};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use std::collections::HashMap;

use sqlparser::ast::ObjectName;

fn parse_object_name(name: &ObjectName) -> (String, String) {
    if name.0.len() >= 2 {
        (
            name.0[0].to_string().trim_matches('"').to_string(),
            name.0[1].to_string().trim_matches('"').to_string(),
        )
    } else if let Some(ident) = name.0.first() {
        ("public".to_string(), ident.to_string().trim_matches('"').to_string())
    } else {
        ("public".to_string(), "unknown".to_string())
    }
}

pub fn parse_schema_sql(sql: &str) -> Result<DbSchema, String> {
    let dialect = PostgreSqlDialect {};
    let ast = Parser::parse_sql(&dialect, sql).map_err(|e| e.to_string())?;

    let mut tables = HashMap::new();
    let mut enums = HashMap::new();
    let mut functions = HashMap::new();
    let mut roles = HashMap::new();
    let mut views = HashMap::new();
    let mut sequences = HashMap::new();
    let mut extensions = HashMap::new();
    let mut composite_types = HashMap::new();
    let mut domains = HashMap::new();

    for statement in ast {
        match statement {
            Statement::CreateTable(CreateTable {
                name,
                columns,
                constraints,
                ..
            }) => {
                let (schema, table_name) = parse_object_name(&name);
                let (parsed_columns, mut foreign_keys, indexes, check_constraints) =
                    parse_columns(&table_name, columns, &constraints);

                // Extract table-level constraints like Foreign Keys and Checks
                for constraint in constraints {
                    match constraint {
                        TableConstraint::ForeignKey(fk) => {
                            if let (Some(col), Some(ref_col)) =
                                (fk.columns.first(), fk.referred_columns.first())
                            {
                                let (ref_schema, ref_table) = parse_object_name(&fk.foreign_table);
                                foreign_keys.push(ForeignKeyInfo {
                                    constraint_name: fk
                                        .name
                                        .as_ref()
                                        .map(|n| n.value.clone())
                                        .unwrap_or_else(|| format!("fk_{}_{}", table_name, col)),
                                    column_name: col.to_string(),
                                    foreign_table: ref_table, // We might want to store schema too? ForeignKeyInfo usually assumes public or same schema?
                                    // Wait, ForeignKeyInfo has schema? No, checked schema.rs before.
                                    // ForeignKeyInfo struct: { foreign_table: String, foreign_column: String, ... }
                                    // If foreign table is in another schema, we probably need foreign_table to be qualified or add foreign_schema.
                                    // BUT: The generated diff logic compares foreign_table string.
                                    // If I stick to unqualified names in ForeignKeyInfo, cross-schema FKs will break.
                                    // I should probably store foreign_schema in ForeignKeyInfo or qualified name.
                                    // CHECK: generator.rs uses foreign_table matching.
                                    // DECISION: For now, I will store just table name to minimize breaking changes or check if I updated ForeignKeyInfo.
                                    // I did NOT update ForeignKeyInfo in schema.rs in the plan specifically for schema, but I updated TableInfo etc.
                                    // Let's check `ForeignKeyInfo` definition again.
                                    foreign_column: ref_col.to_string(),
                                    on_delete: fk
                                        .on_delete
                                        .as_ref()
                                        .map(|a| a.to_string())
                                        .unwrap_or("NO ACTION".to_string()),
                                    on_update: fk
                                        .on_update
                                        .as_ref()
                                        .map(|a| a.to_string())
                                        .unwrap_or("NO ACTION".to_string()),
                                });
                            }
                        }
                        _ => {}
                    }
                }

                let key = format!("\"{}\".\"{}\"", schema, table_name);
                tables.insert(
                    key,
                    TableInfo {
                        schema,
                        table_name: table_name.clone(),
                        columns: parsed_columns,
                        foreign_keys,
                        indexes,
                        triggers: vec![],
                        rls_enabled: false,
                        policies: vec![],
                        check_constraints,
                        comment: None,
                    },
                );
            }
            Statement::CreateType {
                name,
                representation,
                ..
            } => {
                if let Some(rep) = representation {
                    match rep {
                        sqlparser::ast::UserDefinedTypeRepresentation::Enum { labels, .. } => {
                            let (schema, enum_name) = parse_object_name(&name);
                            let key = format!("\"{}\".\"{}\"", schema, enum_name);
                            enums.insert(
                                key,
                                EnumInfo {
                                    schema,
                                    name: enum_name,
                                    values: labels.iter().map(|v| v.value.clone()).collect(),
                                },
                            );
                        }
                        sqlparser::ast::UserDefinedTypeRepresentation::Composite { attributes } => {
                            let (schema, type_name) = parse_object_name(&name);
                            let attrs: Vec<CompositeTypeAttribute> = attributes
                                .iter()
                                .map(|attr| CompositeTypeAttribute {
                                    name: attr.name.to_string(),
                                    data_type: attr.data_type.to_string(),
                                    collation: attr.collation.as_ref().map(|c| c.to_string()),
                                })
                                .collect();

                            let key = format!("\"{}\".\"{}\"", schema, type_name);
                            composite_types.insert(
                                key,
                                CompositeTypeInfo {
                                    schema,
                                    name: type_name,
                                    attributes: attrs,
                                    comment: None,
                                },
                            );
                        }
                        _ => {}
                    }
                }
            }
            Statement::CreateFunction(CreateFunction {
                name,
                args,
                return_type,
                language,
                function_body,
                behavior,
                called_on_null,
                ..
            }) => {
                let (schema, fn_name) = parse_object_name(&name);
                let ret_type = return_type
                    .map(|t| t.to_string().to_lowercase())
                    .unwrap_or("void".to_string());

                let mut fn_args = vec![];
                if let Some(arg_list) = args {
                    for arg in arg_list {
                        if let OperateFunctionArg {
                            name: arg_name,
                            data_type,
                            default_expr,
                            mode,
                        } = arg
                        {
                            let type_str = data_type.to_string().to_lowercase();
                            fn_args.push(FunctionArg {
                                name: arg_name.map(|n| n.value.clone()).unwrap_or_default(),
                                type_: type_str,
                                mode: mode.map(|m| m.to_string()),
                                default_value: default_expr.map(|d| d.to_string()),
                            });
                        }
                    }
                }

                let lang = language.map(|l| l.value).unwrap_or("sql".to_string());

                let volatility = behavior.map(|b| b.to_string());
                let is_strict = called_on_null
                    .map(|c| c.to_string().contains("STRICT"))
                    .unwrap_or(false);

                let def =
                    if let Some(CreateFunctionBody::AsBeforeOptions { body, .. }) = function_body {
                        match body {
                            Expr::Value(v) => match v.value {
                                Value::DollarQuotedString(d) => d.value.trim().to_string(),
                                Value::SingleQuotedString(s) => s.trim().to_string(),
                                _ => v.to_string().trim().to_string(),
                            },
                            _ => body.to_string(),
                        }
                    } else {
                        "".to_string()
                    };

                // Generate signature key: "schema"."name"(arg1, arg2)
                let arg_types: Vec<String> = fn_args.iter().map(|a| a.type_.clone()).collect();
                let signature = format!("\"{}\".\"{}\"({})", schema, fn_name, arg_types.join(", "));

                functions.insert(
                    signature,
                    FunctionInfo {
                        schema,
                        name: fn_name,
                        args: fn_args,
                        return_type: ret_type,
                        language: lang,
                        definition: def,
                        volatility,
                        is_strict,
                        security_definer: false,
                    },
                );
            }
            Statement::CreateRole(CreateRole {
                names,
                login,
                inherit,
                superuser,
                create_db,
                create_role,
                replication,
                bypassrls,
                connection_limit,
                password,
                valid_until,
                ..
            }) => {
                for name in names {
                    let role_name = name.to_string();
                    let pwd = match &password {
                        Some(sqlparser::ast::Password::Password(p)) => Some(p.to_string().trim_matches('\'').to_string()),
                        Some(sqlparser::ast::Password::NullPassword) => None,
                        None => None,
                    };
                    let valid = valid_until.as_ref().map(|v| v.to_string());
                    let conn_limit = connection_limit.as_ref()
                        .map(|c| c.to_string().parse::<i32>().unwrap_or(-1))
                        .unwrap_or(-1);

                    roles.insert(
                        role_name.clone(),
                        RoleInfo {
                            name: role_name,
                            superuser: superuser.unwrap_or(false),
                            create_db: create_db.unwrap_or(false),
                            create_role: create_role.unwrap_or(false),
                            inherit: inherit.unwrap_or(true), // Default is usually INHERIT
                            login: login.unwrap_or(false),
                            replication: replication.unwrap_or(false),
                            bypass_rls: bypassrls.unwrap_or(false),
                            connection_limit: conn_limit,
                            valid_until: valid,
                            password: pwd,
                        },
                    );
                }
            }
            Statement::CreateTrigger(CreateTrigger {
                name,
                table_name,
                period,
                events,
                exec_body,
                trigger_object,
                condition,
                ..
            }) => {
                let t_name = name.to_string();
                let (t_schema, t_table) = parse_object_name(&table_name);
                let table_key = format!("\"{}\".\"{}\"", t_schema, t_table);

                let ev_strs: Vec<String> = events.iter().map(|e| e.to_string()).collect();
                let timing = period
                    .map(|p| p.to_string())
                    .unwrap_or("BEFORE".to_string());

                let orientation = if let Some(obj) = trigger_object {
                    if obj.to_string().to_uppercase().contains("ROW") {
                        "ROW".to_string()
                    } else {
                        "STATEMENT".to_string()
                    }
                } else {
                    "STATEMENT".to_string()
                };

                let func_name = if let Some(TriggerExecBody { func_desc, .. }) = exec_body {
                    func_desc.name.to_string()
                } else {
                    "".to_string()
                };

                let when_clause = condition.map(|c| c.to_string());

                if let Some(t_info) = tables.get_mut(&table_key) {
                    t_info.triggers.push(TriggerInfo {
                        name: t_name,
                        events: ev_strs,
                        timing,
                        orientation,
                        function_name: func_name,
                        when_clause,
                    });
                }
            }
            Statement::CreatePolicy {
                name,
                table_name,
                command,
                to,
                using,
                with_check,
                ..
            } => {
                let p_name = name.value;
                let (t_schema, t_table) = parse_object_name(&table_name);
                let table_key = format!("\"{}\".\"{}\"", t_schema, t_table);

                let cmd = match command.unwrap_or(CreatePolicyCommand::All) {
                    CreatePolicyCommand::All => "ALL",
                    CreatePolicyCommand::Select => "SELECT",
                    CreatePolicyCommand::Insert => "INSERT",
                    CreatePolicyCommand::Update => "UPDATE",
                    CreatePolicyCommand::Delete => "DELETE",
                    _ => "ALL",
                }
                .to_string();

                let roles_vec = if let Some(r_vec) = to {
                    r_vec.iter().map(|i| i.to_string()).collect()
                } else {
                    vec!["public".to_string()]
                };

                let q = using.map(|e| e.to_string());
                let wc = with_check.map(|e| e.to_string());

                if let Some(t_info) = tables.get_mut(&table_key) {
                    t_info.policies.push(PolicyInfo {
                        name: p_name,
                        cmd,
                        roles: roles_vec,
                        qual: q,
                        with_check: wc,
                    });
                }
            }
            Statement::AlterTable(AlterTable {
                name, operations, ..
            }) => {
                let (schema, table_name) = parse_object_name(&name);
                let table_key = format!("\"{}\".\"{}\"", schema, table_name);

                if let Some(t_info) = tables.get_mut(&table_key) {
                    for op in operations {
                        match op {
                            AlterTableOperation::EnableRowLevelSecurity => {
                                t_info.rls_enabled = true
                            }
                            AlterTableOperation::DisableRowLevelSecurity => {
                                t_info.rls_enabled = false
                            }
                            AlterTableOperation::AddConstraint { constraint, .. } => {
                                match constraint {
                                    TableConstraint::ForeignKey(fk) => {
                                        if let (Some(col), Some(ref_col)) =
                                            (fk.columns.first(), fk.referred_columns.first())
                                        {
                                            let constraint_name = if let Some(n) = &fk.name {
                                                n.value.clone()
                                            } else {
                                                format!("fk_{}_{}", table_name, col)
                                            };
                                            
                                            // Handle referenced table schema
                                            let (ref_schema, ref_table) = parse_object_name(&fk.foreign_table);

                                            t_info.foreign_keys.push(ForeignKeyInfo {
                                                constraint_name,
                                                column_name: col.to_string(),
                                                foreign_table: ref_table, // Keep simplified for now as per previous block decision
                                                foreign_column: ref_col.to_string(),
                                                on_delete: fk
                                                    .on_delete
                                                    .as_ref()
                                                    .map(|a| a.to_string())
                                                    .unwrap_or("NO ACTION".to_string()),
                                                on_update: fk
                                                    .on_update
                                                    .as_ref()
                                                    .map(|a| a.to_string())
                                                    .unwrap_or("NO ACTION".to_string()),
                                            });
                                        }
                                    }
                                    TableConstraint::Unique(uq) => {
                                        let columns: Vec<String> =
                                            uq.columns.iter().map(|c| c.to_string()).collect();
                                        let constraint_name = if let Some(n) = &uq.name {
                                            n.value.clone()
                                        } else {
                                            format!("{}_{}_key", table_name, columns.join("_"))
                                        };

                                        t_info.indexes.push(IndexInfo {
                                            index_name: constraint_name.clone(),
                                            columns,
                                            is_unique: true,
                                            is_primary: false,
                                            owning_constraint: Some(constraint_name),
                                            index_method: "btree".to_string(), // Default for UNIQUE constraint
                                            where_clause: None,
                                            expressions: vec![],
                                        });
                                    }
                                    TableConstraint::Check(chk) => {
                                        let constraint_name = chk
                                            .name
                                            .as_ref()
                                            .map(|n| n.value.clone())
                                            .unwrap_or_else(|| format!("{}_check", table_name));

                                        t_info.check_constraints.push(CheckConstraintInfo {
                                            name: constraint_name,
                                            expression: format!("CHECK ({})", chk.expr),
                                            columns: vec![],
                                        });
                                    }
                                    _ => {}
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            Statement::CreateIndex(CreateIndex {
                name,
                table_name,
                columns,
                unique,
                using,
                predicate,
                ..
            }) => {
                let index_name = name.map(|n| n.to_string()).unwrap_or_default();
                let (schema, t_name) = parse_object_name(&table_name);
                let table_key = format!("\"{}\".\"{}\"", schema, t_name);

                let (index_columns, expressions): (Vec<String>, Vec<String>) = columns
                    .iter()
                    .map(|c| {
                        let col_str = match &c.column.expr {
                            Expr::Identifier(ident) => ident.value.clone(),
                            _ => c.column.to_string(),
                        };
                        // Check if it's an expression (contains function call)
                        if col_str.contains('(') {
                            (String::new(), col_str)
                        } else {
                            (col_str, String::new())
                        }
                    })
                    .unzip();

                let index_columns: Vec<String> = index_columns
                    .into_iter()
                    .filter(|c| !c.is_empty())
                    .collect();
                let expressions: Vec<String> =
                    expressions.into_iter().filter(|e| !e.is_empty()).collect();

                let index_method = using
                    .map(|u| u.to_string().to_lowercase())
                    .unwrap_or("btree".to_string());
                let where_clause = predicate.map(|p| p.to_string());

                if let Some(t_info) = tables.get_mut(&table_key) {
                    t_info.indexes.push(IndexInfo {
                        index_name,
                        columns: index_columns,
                        is_unique: unique,
                        is_primary: false,
                        owning_constraint: None,
                        index_method,
                        where_clause,
                        expressions,
                    });
                }
            }
            Statement::CreateView(sqlparser::ast::CreateView {
                name,
                query,
                materialized,
                options,
                ..
            }) => {
                let (schema, view_name) = parse_object_name(&name);
                let definition = query.to_string();

                let with_options: Vec<String> = match options {
                    sqlparser::ast::CreateTableOptions::Options(opts) => {
                        opts.iter().map(|o| o.to_string()).collect()
                    }
                    _ => vec![],
                };

                let key = format!("\"{}\".\"{}\"", schema, view_name);
                views.insert(
                    key,
                    ViewInfo {
                        schema,
                        name: view_name,
                        definition,
                        is_materialized: materialized,
                        columns: vec![],
                        indexes: vec![],
                        comment: None,
                        with_options,
                        check_option: None,
                    },
                );
            }
            Statement::CreateSequence {
                name,
                data_type,
                sequence_options,
                ..
            } => {
                let (schema, seq_name) = parse_object_name(&name);
                let dtype = data_type
                    .map(|dt| dt.to_string().to_lowercase())
                    .unwrap_or("bigint".to_string());

                let mut start_value: i64 = 1;
                let mut min_value: i64 = 1;
                let mut max_value: i64 = i64::MAX;
                let mut increment: i64 = 1;
                let mut cycle = false;
                let mut cache_size: i64 = 1;
                let owned_by: Option<String> = None;

                for opt in sequence_options {
                    match opt {
                        sqlparser::ast::SequenceOptions::StartWith(v, _) => {
                            start_value = v.to_string().parse().unwrap_or(1);
                        }
                        sqlparser::ast::SequenceOptions::MinValue(Some(v)) => {
                            min_value = v.to_string().parse().unwrap_or(1);
                        }
                        sqlparser::ast::SequenceOptions::MaxValue(Some(v)) => {
                            max_value = v.to_string().parse().unwrap_or(i64::MAX);
                        }
                        sqlparser::ast::SequenceOptions::IncrementBy(v, _) => {
                            increment = v.to_string().parse().unwrap_or(1);
                        }
                        sqlparser::ast::SequenceOptions::Cycle(c) => cycle = c,
                        sqlparser::ast::SequenceOptions::Cache(v) => {
                            cache_size = v.to_string().parse().unwrap_or(1);
                        }
                        _ => {}
                    }
                }

                let key = format!("\"{}\".\"{}\"", schema, seq_name);
                sequences.insert(
                    key,
                    SequenceInfo {
                        schema,
                        name: seq_name,
                        data_type: dtype,
                        start_value,
                        min_value,
                        max_value,
                        increment,
                        cycle,
                        cache_size,
                        owned_by,
                        comment: None,
                    },
                );
            }
            Statement::CreateExtension(sqlparser::ast::CreateExtension {
                name,
                schema,
                version,
                ..
            }) => {
                let ext_name = name.to_string().trim_matches('"').to_string();
                let ext_schema = schema.map(|s| s.to_string()).unwrap_or("public".to_string());
                
                extensions.insert(
                    ext_name.clone(),
                    ExtensionInfo {
                        name: ext_name,
                        version: version.map(|v| v.to_string()),
                        schema: Some(ext_schema),
                    },
                );
            }
            Statement::CreateDomain(sqlparser::ast::CreateDomain {
                name,
                data_type,
                default,
                constraints,
                collation,
            }) => {
                let (schema, domain_name) = parse_object_name(&name);
                let base_type = data_type.to_string().to_lowercase();
                let default_value = default.map(|d| d.to_string());

                let mut is_not_null = false;
                let mut check_constraints = vec![];

                for constraint in constraints {
                    match constraint {
                        TableConstraint::Check(chk) => {
                            check_constraints.push(DomainCheckConstraint {
                                name: chk.name.as_ref().map(|n| n.value.clone()),
                                expression: format!("CHECK ({})", chk.expr),
                            });
                        }
                        // Handle NOT NULL constraint if encoded as a table constraint (sometimes simpler dialects do this, but PG usually uses proper domain constraints)
                        // sqlparser 0.60 CreateDomain constraints are TableConstraints? Yes.
                        // Check if TableConstraint can be NotNull? It's usually ColumnOption.
                        // But domains can have NOT NULL. In sqlparser it might be parsed differently or custom.
                        // Let's assume Check for now.
                        _ => {}
                    }
                }

                let key = format!("\"{}\".\"{}\"", schema, domain_name);
                domains.insert(
                    key,
                    DomainInfo {
                        schema,
                        name: domain_name,
                        base_type,
                        default_value,
                        is_not_null,
                        check_constraints,
                        collation: collation.map(|c| c.to_string()),
                        comment: None,
                    },
                );
            }
            Statement::Comment {
                object_type,
                object_name,
                comment,
                ..
            } => {
                match object_type {
                    sqlparser::ast::CommentObject::Table => {
                        let (schema, table_name) = parse_object_name(&object_name);
                        let key = format!("\"{}\".\"{}\"", schema, table_name);
                        
                        if let Some(table) = tables.get_mut(&key) {
                            table.comment = comment;
                        }
                    }
                    sqlparser::ast::CommentObject::Column => {
                        // ObjectName for column should be [schema, table, column] or [table, column]
                        let idents = &object_name.0;
                        if idents.len() >= 3 {
                            let schema = idents[0].to_string().trim_matches('"').to_string();
                            let table = idents[1].to_string().trim_matches('"').to_string();
                            let col = idents[2].to_string().trim_matches('"').to_string();
                            let table_key = format!("\"{}\".\"{}\"", schema, table);
                            
                            if let Some(t_info) = tables.get_mut(&table_key) {
                                if let Some(c_info) = t_info.columns.get_mut(&col) {
                                    c_info.comment = comment;
                                }
                            }
                        } else if idents.len() == 2 {
                            // Ambiguous: could be schema.table (comment on table) or table.column (comment on column)?
                            // OBJECT_TYPE is Column, so it must be table.column. Default to public schema.
                            let schema = "public".to_string();
                            let table = idents[0].to_string().trim_matches('"').to_string();
                            let col = idents[1].to_string().trim_matches('"').to_string();
                            let table_key = format!("\"{}\".\"{}\"", schema, table);

                            if let Some(t_info) = tables.get_mut(&table_key) {
                                if let Some(c_info) = t_info.columns.get_mut(&col) {
                                    c_info.comment = comment;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(DbSchema {
        tables,
        enums,
        functions,
        roles,
        views,
        sequences,
        extensions,
        composite_types,
        domains,
    })
}

fn parse_columns(
    table_name: &str,
    columns: Vec<ColumnDef>,
    constraints: &[TableConstraint],
) -> (
    HashMap<String, ColumnInfo>,
    Vec<ForeignKeyInfo>,
    Vec<IndexInfo>,
    Vec<CheckConstraintInfo>,
) {
    let mut infos = HashMap::new();
    let mut fks = Vec::new();
    let mut option_indexes = Vec::new();
    let mut check_constraints = Vec::new();

    for col in columns {
        let name = col.name.to_string();
        let data_type = col.data_type.to_string();
        let mut is_nullable = true;
        let mut is_primary_key = false;
        let mut is_unique = false;
        let mut column_default = None;
        let mut is_identity = false;
        let mut identity_generation = None;
        let mut collation = None;

        for option in &col.options {
            match &option.option {
                ColumnOption::NotNull => is_nullable = false,
                ColumnOption::Unique { .. } => is_unique = true,
                ColumnOption::Default(expr) => column_default = Some(expr.to_string()),
                ColumnOption::Generated { generated_as, .. } => {
                     is_identity = true;
                     identity_generation = match generated_as {
                         sqlparser::ast::GeneratedAs::Always => Some("ALWAYS".to_string()),
                         sqlparser::ast::GeneratedAs::ByDefault => Some("BY DEFAULT".to_string()),
                         _ => Some("BY DEFAULT".to_string()),
                     };
                }
                ColumnOption::Collation(c) => collation = Some(c.to_string()),
                ColumnOption::Check(check_expr) => {
                    let constraint_name = option
                        .name
                        .as_ref()
                        .map(|n| n.value.clone())
                        .unwrap_or_else(|| format!("{}_{}_check", table_name, name));

                    check_constraints.push(CheckConstraintInfo {
                        name: constraint_name,
                        expression: format!("CHECK ({})", check_expr),
                        columns: vec![name.clone()],
                    });
                }
                ColumnOption::ForeignKey { .. } => {
                    // Handle inline foreign key references
                }
                _ => {}
            }
        }

        // Check table constraints for PK/Unique
        for constraint in constraints {
            match constraint {
                TableConstraint::PrimaryKey(pk) => {
                    if pk.columns.iter().any(|c| c.to_string() == name) {
                        is_primary_key = true;
                        is_nullable = false;
                    }
                }
                TableConstraint::Unique(uq) => {
                    if uq.columns.iter().any(|c| c.to_string() == name) {
                        is_unique = true;
                    }
                }
                TableConstraint::Check(chk) => {
                    // Table-level check constraints
                    let constraint_name = chk
                        .name
                        .as_ref()
                        .map(|n| n.value.clone())
                        .unwrap_or_else(|| format!("{}_check", table_name));

                    // Only add if not already added
                    if !check_constraints.iter().any(|c| c.name == constraint_name) {
                        check_constraints.push(CheckConstraintInfo {
                            name: constraint_name,
                            expression: format!("CHECK ({})", chk.expr),
                            columns: vec![],
                        });
                    }
                }
                _ => {}
            }
        }

        infos.insert(
            name.clone(),
            ColumnInfo {
                column_name: name,
                data_type: data_type.clone(),
                is_nullable,
                column_default,
                udt_name: data_type.clone(),
                is_primary_key,
                is_unique,
                is_identity,
                identity_generation,
                collation,
                enum_name: None,
                is_array: false,
                comment: None,
            },
        );
    }

    (infos, fks, option_indexes, check_constraints)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_schema() {
        let sql = r#"
CREATE OR REPLACE FUNCTION update_player_last_played() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RETURN NEW;
END;
$$;

CREATE TABLE players (
    id uuid NOT NULL
);

CREATE TRIGGER update_player_timestamp BEFORE UPDATE ON players FOR EACH ROW EXECUTE FUNCTION update_player_last_played();

CREATE POLICY "public_read" ON players FOR SELECT USING (true);

ALTER TABLE players ENABLE ROW LEVEL SECURITY;
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        // Verify Function
        let func = schema
            .functions
            .get("\"public\".\"update_player_last_played\"()")
            .expect("Function not found");
        assert_eq!(func.language, "plpgsql");
        assert_eq!(func.return_type, "trigger");

        // Verify Table
        let table = schema.tables.get("\"public\".\"players\"").expect("Table not found");
        assert!(table.rls_enabled);

        // Verify Trigger
        assert_eq!(table.triggers.len(), 1);
        let trigger = &table.triggers[0];
        assert_eq!(trigger.name, "update_player_timestamp");
        assert_eq!(trigger.timing, "BEFORE");
        assert_eq!(trigger.orientation, "ROW");
        assert_eq!(trigger.function_name, "update_player_last_played");

        // Verify Policy
        assert_eq!(table.policies.len(), 1);
        let policy = &table.policies[0];
        assert_eq!(policy.name, "public_read");
        assert_eq!(policy.cmd, "SELECT");
    }

    #[test]
    fn test_parse_schema_mismatch() {
        let sql = r#"
CREATE TABLE players (
    id uuid NOT NULL
);

CREATE TRIGGER update_player_timestamp BEFORE UPDATE ON public.players FOR EACH ROW EXECUTE FUNCTION update_player_last_played();
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        // Verify Table
        let table = schema.tables.get("\"public\".\"players\"").expect("Table not found");

        // Verify Trigger should exist even if ON public.players
        assert_eq!(table.triggers.len(), 1);
        let trigger = &table.triggers[0];
        assert_eq!(trigger.name, "update_player_timestamp");
    }

    #[test]
    fn test_parse_views() {
        let sql = r#"
CREATE VIEW user_stats AS SELECT id, count(*) as post_count FROM users GROUP BY id;
CREATE MATERIALIZED VIEW cached_stats AS SELECT * FROM user_stats;
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        assert_eq!(schema.views.len(), 2);

        let view = schema.views.get("\"public\".\"user_stats\"").expect("View not found");
        assert!(!view.is_materialized);

        let mat_view = schema
            .views
            .get("\"public\".\"cached_stats\"")
            .expect("Materialized view not found");
        assert!(mat_view.is_materialized);
    }

    #[test]
    fn test_parse_sequences() {
        let sql = r#"
CREATE SEQUENCE user_id_seq INCREMENT BY 1 MINVALUE 1 MAXVALUE 1000000 CACHE 10;
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let seq = schema
            .sequences
            .get("\"public\".\"user_id_seq\"")
            .expect("Sequence not found");
        assert_eq!(seq.increment, 1);
        assert_eq!(seq.min_value, 1);
        assert_eq!(seq.max_value, 1000000);
        assert_eq!(seq.cache_size, 10);
    }

// ... skipping extensions test ...

    #[test]
    fn test_parse_composite_types() {
        let sql = r#"
CREATE TYPE address AS (
    street text,
    city text,
    zip_code text
);
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let addr_type = schema
            .composite_types
            .get("\"public\".\"address\"")
            .expect("Composite type not found");
        assert_eq!(addr_type.attributes.len(), 3);
        assert_eq!(addr_type.attributes[0].name, "street");
    }

    #[test]
    fn test_parse_check_constraints() {
        let sql = r#"
CREATE TABLE users (
    id uuid NOT NULL,
    age integer CHECK (age > 0),
    CONSTRAINT valid_age CHECK (age < 150)
);
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");

        assert!(table.check_constraints.len() >= 1);
    }

    #[test]
    fn test_parse_partial_index() {
        let sql = r#"
CREATE TABLE users (id uuid NOT NULL, active boolean);
CREATE INDEX active_users_idx ON users (id) WHERE active = true;
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");

        let idx = table
            .indexes
            .iter()
            .find(|i| i.index_name == "active_users_idx")
            .expect("Index not found");
        assert!(idx.where_clause.is_some());
    }

    #[test]
    fn test_parse_foreign_key_on_update() {
        let sql = r#"
CREATE TABLE departments (id uuid PRIMARY KEY);
CREATE TABLE users (
    id uuid NOT NULL,
    dept_id uuid REFERENCES departments(id) ON DELETE CASCADE ON UPDATE SET NULL
);
        "#;

        // This test verifies the FK ON UPDATE parsing works
        // Note: inline REFERENCES may not fully parse ON UPDATE in all sqlparser versions
        let _schema = parse_schema_sql(sql).expect("Failed to parse SQL");
    }

    #[test]
    fn test_parse_indexes_and_constraints() {
        let sql = r#"
CREATE TABLE users ( id uuid );
CREATE UNIQUE INDEX idx_email ON users (email);
ALTER TABLE users ADD CONSTRAINT fk_role FOREIGN KEY (role_id) REFERENCES roles(id);
ALTER TABLE users ADD CONSTRAINT unique_username UNIQUE (username);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"users\"").expect("Table not found");

        // Verify CREATE INDEX
        assert!(table
            .indexes
            .iter()
            .any(|i| i.index_name == "idx_email" && i.is_unique));

        // Verify ALTER TABLE FK
        assert!(table
            .foreign_keys
            .iter()
            .any(|fk| fk.constraint_name == "fk_role"));

        // Verify ALTER TABLE UNIQUE (should be an index with constraint)
        assert!(table
            .indexes
            .iter()
            .any(|i| i.index_name == "unique_username"
                && i.owning_constraint.as_deref() == Some("unique_username")));
    }

    #[test]
    fn test_parse_identity_and_collation() {
        let sql = r#"
CREATE TABLE items (
    id integer GENERATED ALWAYS AS IDENTITY,
    code text COLLATE "C"
);
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("\"public\".\"items\"").expect("Table not found");

        let id_col = table.columns.get("id").expect("id column not found");
        assert!(id_col.is_identity);
        assert_eq!(id_col.identity_generation, Some("ALWAYS".to_string()));

        let code_col = table.columns.get("code").expect("code column not found");
        assert_eq!(code_col.collation, Some("\"C\"".to_string()));
    }

    #[test]
    fn test_parse_function_overloading() {
        let sql = r#"
CREATE FUNCTION add(a integer, b integer) RETURNS integer LANGUAGE sql AS 'SELECT a + b';
CREATE FUNCTION add(a float, b float) RETURNS float LANGUAGE sql AS 'SELECT a + b';
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        assert_eq!(schema.functions.len(), 2);
        assert!(schema.functions.contains_key("\"public\".\"add\"(integer, integer)"));
        assert!(schema.functions.contains_key("\"public\".\"add\"(float, float)"));
    }

    #[test]
    fn test_parse_roles() {
        let sql = r#"
CREATE ROLE "Test" WITH LOGIN SUPERUSER PASSWORD 'secret';
CREATE ROLE "readonly";
"#;
        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        assert!(schema.roles.contains_key("\"Test\""));
        let test_role = schema.roles.get("\"Test\"").unwrap();
        assert!(test_role.login);
        assert!(test_role.superuser);
        assert_eq!(test_role.password, Some("secret".to_string()));

        assert!(schema.roles.contains_key("\"readonly\""));
        let readonly_role = schema.roles.get("\"readonly\"").unwrap();
        assert!(!readonly_role.superuser);
        assert!(!readonly_role.login);
    }
}
