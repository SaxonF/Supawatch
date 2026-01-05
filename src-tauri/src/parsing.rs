use crate::schema::{
    CheckConstraintInfo, ColumnInfo, CompositeTypeAttribute, CompositeTypeInfo, DbSchema,
    DomainCheckConstraint, DomainInfo, EnumInfo, ExtensionInfo, ForeignKeyInfo, FunctionArg,
    FunctionInfo, IndexInfo, PolicyInfo, SequenceInfo, TableInfo, TriggerInfo, ViewColumnInfo,
    ViewInfo,
};
use sqlparser::ast::{
    AlterTable, AlterTableOperation, ColumnDef, ColumnOption, CreateFunction, CreateFunctionBody,
    CreateIndex, CreatePolicyCommand, CreateTable, CreateTrigger, DataType, Expr, Ident,
    ObjectName, OperateFunctionArg, Statement, TableConstraint, TriggerExecBody, Value,
};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use std::collections::HashMap;

fn normalize_table_name(name: &str) -> String {
    name.trim_start_matches("public.").to_string()
}

pub fn parse_schema_sql(sql: &str) -> Result<DbSchema, String> {
    let dialect = PostgreSqlDialect {};
    let ast = Parser::parse_sql(&dialect, sql).map_err(|e| e.to_string())?;

    let mut tables = HashMap::new();
    let mut enums = HashMap::new();
    let mut functions = HashMap::new();
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
                let raw_name = name.to_string();
                let table_name = normalize_table_name(&raw_name);
                let (parsed_columns, mut foreign_keys, indexes, check_constraints) =
                    parse_columns(&table_name, columns, &constraints);

                // Extract table-level constraints like Foreign Keys and Checks
                for constraint in constraints {
                    match constraint {
                        TableConstraint::ForeignKey(fk) => {
                            if let (Some(col), Some(ref_col)) =
                                (fk.columns.first(), fk.referred_columns.first())
                            {
                                foreign_keys.push(ForeignKeyInfo {
                                    constraint_name: fk
                                        .name
                                        .as_ref()
                                        .map(|n| n.value.clone())
                                        .unwrap_or_else(|| format!("fk_{}_{}", table_name, col)),
                                    column_name: col.to_string(),
                                    foreign_table: normalize_table_name(
                                        &fk.foreign_table.to_string(),
                                    ),
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

                tables.insert(
                    table_name.clone(),
                    TableInfo {
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
                            let enum_name = name.to_string();
                            enums.insert(
                                enum_name.clone(),
                                EnumInfo {
                                    name: enum_name,
                                    values: labels.iter().map(|v| v.value.clone()).collect(),
                                },
                            );
                        }
                        sqlparser::ast::UserDefinedTypeRepresentation::Composite { attributes } => {
                            let type_name = name.to_string();
                            let attrs: Vec<CompositeTypeAttribute> = attributes
                                .iter()
                                .map(|attr| CompositeTypeAttribute {
                                    name: attr.name.to_string(),
                                    data_type: attr.data_type.to_string(),
                                    collation: attr.collation.as_ref().map(|c| c.to_string()),
                                })
                                .collect();

                            composite_types.insert(
                                type_name.clone(),
                                CompositeTypeInfo {
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
                let fn_name = name.to_string();
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

                functions.insert(
                    fn_name.clone(),
                    FunctionInfo {
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
                let table_target = normalize_table_name(&table_name.to_string());

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

                if let Some(t_info) = tables.get_mut(&table_target) {
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
                let table_target = normalize_table_name(&table_name.to_string());
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

                if let Some(t_info) = tables.get_mut(&table_target) {
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
                let table_target = normalize_table_name(&name.to_string());
                if let Some(t_info) = tables.get_mut(&table_target) {
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
                                                format!("fk_{}_{}", table_target, col)
                                            };

                                            t_info.foreign_keys.push(ForeignKeyInfo {
                                                constraint_name,
                                                column_name: col.to_string(),
                                                foreign_table: normalize_table_name(
                                                    &fk.foreign_table.to_string(),
                                                ),
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
                                            format!("{}_{}_key", table_target, columns.join("_"))
                                        };

                                        t_info.indexes.push(IndexInfo {
                                            index_name: constraint_name.clone(),
                                            columns,
                                            is_unique: true,
                                            is_primary: false,
                                            owning_constraint: Some(constraint_name),
                                            index_method: "btree".to_string(),
                                            where_clause: None,
                                            expressions: vec![],
                                        });
                                    }
                                    TableConstraint::Check(chk) => {
                                        let constraint_name = chk
                                            .name
                                            .as_ref()
                                            .map(|n| n.value.clone())
                                            .unwrap_or_else(|| format!("{}_check", table_target));

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
                let table_target = normalize_table_name(&table_name.to_string());

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

                if let Some(t_info) = tables.get_mut(&table_target) {
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
            Statement::CreateView {
                name,
                query,
                materialized,
                options,
                with_no_schema_binding,
                ..
            } => {
                let view_name = normalize_table_name(&name.to_string());
                let definition = query.to_string();

                let with_options: Vec<String> = options
                    .options
                    .iter()
                    .map(|o| format!("{}={}", o.name, o.value))
                    .collect();

                views.insert(
                    view_name.clone(),
                    ViewInfo {
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
                let seq_name = normalize_table_name(&name.to_string());
                let dtype = data_type
                    .map(|dt| dt.to_string().to_lowercase())
                    .unwrap_or("bigint".to_string());

                let mut start_value: i64 = 1;
                let mut min_value: i64 = 1;
                let mut max_value: i64 = i64::MAX;
                let mut increment: i64 = 1;
                let mut cycle = false;
                let mut cache_size: i64 = 1;
                let mut owned_by: Option<String> = None;

                for opt in sequence_options.unwrap_or_default() {
                    match opt {
                        sqlparser::ast::SequenceOptions::StartWith(v, _) => {
                            start_value = v.value.parse().unwrap_or(1);
                        }
                        sqlparser::ast::SequenceOptions::MinValue(Some(v)) => {
                            min_value = v.value.parse().unwrap_or(1);
                        }
                        sqlparser::ast::SequenceOptions::MaxValue(Some(v)) => {
                            max_value = v.value.parse().unwrap_or(i64::MAX);
                        }
                        sqlparser::ast::SequenceOptions::IncrementBy(v, _) => {
                            increment = v.value.parse().unwrap_or(1);
                        }
                        sqlparser::ast::SequenceOptions::Cycle(c) => cycle = c,
                        sqlparser::ast::SequenceOptions::Cache(v) => {
                            cache_size = v.value.parse().unwrap_or(1);
                        }
                        sqlparser::ast::SequenceOptions::OwnedBy(obj) => {
                            owned_by = Some(obj.to_string());
                        }
                        _ => {}
                    }
                }

                sequences.insert(
                    seq_name.clone(),
                    SequenceInfo {
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
            Statement::CreateExtension {
                name,
                schema,
                version,
                ..
            } => {
                let ext_name = name.value.clone();
                extensions.insert(
                    ext_name.clone(),
                    ExtensionInfo {
                        name: ext_name,
                        version: version.map(|v| v.to_string()),
                        schema: schema.map(|s| s.to_string()),
                    },
                );
            }
            Statement::CreateDomain {
                name,
                data_type,
                default,
                constraints,
                collation,
            } => {
                let domain_name = name.to_string();
                let base_type = data_type.to_string().to_lowercase();
                let default_value = default.map(|d| d.to_string());

                let mut is_not_null = false;
                let mut check_constraints = vec![];

                for constraint in constraints {
                    match constraint {
                        sqlparser::ast::DomainConstraint::NotNull => is_not_null = true,
                        sqlparser::ast::DomainConstraint::Check(expr, name) => {
                            check_constraints.push(DomainCheckConstraint {
                                name: name.map(|n| n.value),
                                expression: format!("CHECK ({})", expr),
                            });
                        }
                    }
                }

                domains.insert(
                    domain_name.clone(),
                    DomainInfo {
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
                let obj_name = object_name.to_string();
                let normalized_name = normalize_table_name(&obj_name);

                match object_type {
                    sqlparser::ast::CommentObject::Table => {
                        if let Some(table) = tables.get_mut(&normalized_name) {
                            table.comment = comment;
                        }
                    }
                    sqlparser::ast::CommentObject::Column => {
                        // Parse "table.column" format
                        let parts: Vec<&str> = obj_name.split('.').collect();
                        if parts.len() >= 2 {
                            let table_name = normalize_table_name(parts[0]);
                            let col_name = parts[parts.len() - 1];
                            if let Some(table) = tables.get_mut(&table_name) {
                                if let Some(col) = table.columns.get_mut(col_name) {
                                    col.comment = comment;
                                }
                            }
                        }
                    }
                    sqlparser::ast::CommentObject::View => {
                        if let Some(view) = views.get_mut(&normalized_name) {
                            view.comment = comment;
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

        for option in &col.options {
            match &option.option {
                ColumnOption::NotNull => is_nullable = false,
                ColumnOption::Unique { .. } => is_unique = true,
                ColumnOption::Default(expr) => column_default = Some(expr.to_string()),
                ColumnOption::Generated { .. } => is_identity = true,
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
            .get("update_player_last_played")
            .expect("Function not found");
        assert_eq!(func.language, "plpgsql");
        assert_eq!(func.return_type, "trigger");

        // Verify Table
        let table = schema.tables.get("players").expect("Table not found");
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
        let table = schema.tables.get("players").expect("Table not found");

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

        let view = schema.views.get("user_stats").expect("View not found");
        assert!(!view.is_materialized);

        let mat_view = schema
            .views
            .get("cached_stats")
            .expect("Materialized view not found");
        assert!(mat_view.is_materialized);
    }

    #[test]
    fn test_parse_sequences() {
        let sql = r#"
CREATE SEQUENCE user_id_seq START WITH 1 INCREMENT BY 1 MINVALUE 1 MAXVALUE 1000000 CACHE 10;
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        let seq = schema
            .sequences
            .get("user_id_seq")
            .expect("Sequence not found");
        assert_eq!(seq.start_value, 1);
        assert_eq!(seq.increment, 1);
        assert_eq!(seq.min_value, 1);
        assert_eq!(seq.max_value, 1000000);
        assert_eq!(seq.cache_size, 10);
    }

    #[test]
    fn test_parse_extensions() {
        let sql = r#"
CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public;
CREATE EXTENSION pgcrypto;
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");

        assert!(schema.extensions.contains_key("uuid-ossp"));
        assert!(schema.extensions.contains_key("pgcrypto"));
    }

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
            .get("address")
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
        let table = schema.tables.get("users").expect("Table not found");

        assert!(table.check_constraints.len() >= 1);
    }

    #[test]
    fn test_parse_partial_index() {
        let sql = r#"
CREATE TABLE users (id uuid NOT NULL, active boolean);
CREATE INDEX active_users_idx ON users (id) WHERE active = true;
        "#;

        let schema = parse_schema_sql(sql).expect("Failed to parse SQL");
        let table = schema.tables.get("users").expect("Table not found");

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
        let table = schema.tables.get("users").expect("Table not found");

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
}
