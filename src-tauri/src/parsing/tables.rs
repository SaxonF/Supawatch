use crate::schema::{
    CheckConstraintInfo, ColumnInfo, ForeignKeyInfo, IndexInfo, TableInfo,
};
use sqlparser::ast::{
    AlterTable, AlterTableOperation, ColumnDef, ColumnOption, CreateIndex, CreateTable,
    TableConstraint, Expr,
};
use std::collections::HashMap;
use super::helpers::{parse_object_name, strip_quotes};

pub fn handle_create_table(
    tables: &mut HashMap<String, TableInfo>,
    stmt: CreateTable,
) {
    let CreateTable {
        name,
        columns,
        constraints,
        ..
    } = stmt;

    let (schema, table_name) = parse_object_name(&name);
    let (parsed_columns, mut foreign_keys, indexes, mut check_constraints) =
        parse_columns(&table_name, columns, &constraints);

    // Extract table-level constraints like Foreign Keys and Checks
    for constraint in constraints {
        match constraint {
            TableConstraint::ForeignKey(fk) => {
                if let (Some(col), Some(ref_col)) =
                    (fk.columns.first(), fk.referred_columns.first())
                {
                    let (_, ref_table) = parse_object_name(&fk.foreign_table);
                    foreign_keys.push(ForeignKeyInfo {
                        constraint_name: fk
                            .name
                            .as_ref()
                            .map(|n| strip_quotes(&n.value))
                            .unwrap_or_else(|| format!("fk_{}_{}", table_name, strip_quotes(&col.to_string()))),
                        column_name: strip_quotes(&col.to_string()),
                        foreign_table: ref_table,
                        foreign_column: strip_quotes(&ref_col.to_string()),
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
            TableConstraint::Check(chk) => {
                let constraint_name = chk
                    .name
                    .as_ref()
                    .map(|n| strip_quotes(&n.value))
                    .unwrap_or_else(|| format!("{}_check", table_name));

                check_constraints.push(CheckConstraintInfo {
                    name: constraint_name,
                    expression: format!("CHECK ({})", chk.expr),
                    columns: vec![],
                });
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

pub fn handle_alter_table(
    tables: &mut HashMap<String, TableInfo>,
    stmt: AlterTable,
) {
    let AlterTable { name, operations, .. } = stmt;
    let (schema, table_name) = parse_object_name(&name);
    let table_key = format!("\"{}\".\"{}\"", schema, table_name);

    if let Some(t_info) = tables.get_mut(&table_key) {
        for op in operations {
            match op {
                AlterTableOperation::EnableRowLevelSecurity => t_info.rls_enabled = true,
                AlterTableOperation::DisableRowLevelSecurity => t_info.rls_enabled = false,
                AlterTableOperation::AddConstraint { constraint, .. } => {
                    match constraint {
                        TableConstraint::ForeignKey(fk) => {
                            if let (Some(col), Some(ref_col)) =
                                (fk.columns.first(), fk.referred_columns.first())
                            {
                                let constraint_name = if let Some(n) = &fk.name {
                                    strip_quotes(&n.value)
                                } else {
                                    format!("fk_{}_{}", table_name, strip_quotes(&col.to_string()))
                                };
                                let (_, ref_table) = parse_object_name(&fk.foreign_table);

                                t_info.foreign_keys.push(ForeignKeyInfo {
                                    constraint_name,
                                    column_name: strip_quotes(&col.to_string()),
                                    foreign_table: ref_table,
                                    foreign_column: strip_quotes(&ref_col.to_string()),
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
                                uq.columns.iter().map(|c| strip_quotes(&c.to_string())).collect();
                            let constraint_name = if let Some(n) = &uq.name {
                                strip_quotes(&n.value)
                            } else {
                                format!("{}_{}_key", table_name, columns.join("_"))
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
                                .map(|n| strip_quotes(&n.value))
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

pub fn handle_create_index(
    tables: &mut HashMap<String, TableInfo>,
    stmt: CreateIndex,
) {
    let CreateIndex {
        name,
        table_name,
        columns,
        unique,
        using,
        predicate,
        ..
    } = stmt;

    let index_name = name.map(|n| strip_quotes(&n.to_string())).unwrap_or_default();
    let (schema, t_name) = parse_object_name(&table_name);
    let table_key = format!("\"{}\".\"{}\"", schema, t_name);

    let (index_columns, expressions): (Vec<String>, Vec<String>) = columns
        .iter()
        .map(|c| {
            let col_str = match &c.column.expr {
                Expr::Identifier(ident) => strip_quotes(&ident.value),
                _ => strip_quotes(&c.column.to_string()),
            };
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
    let expressions: Vec<String> = expressions.into_iter().filter(|e| !e.is_empty()).collect();

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

pub fn handle_comment(
    tables: &mut HashMap<String, TableInfo>,
    object_type: sqlparser::ast::CommentObject,
    object_name: sqlparser::ast::ObjectName,
    comment: Option<String>,
) {

    match object_type {
        sqlparser::ast::CommentObject::Table => {
            let (schema, table_name) = parse_object_name(&object_name);
            let key = format!("\"{}\".\"{}\"", schema, table_name);
            if let Some(table) = tables.get_mut(&key) {
                table.comment = comment;
            }
        }
        sqlparser::ast::CommentObject::Column => {
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

pub fn parse_columns(
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
    let fks = Vec::new();
    let _option_indexes: Vec<IndexInfo> = Vec::new(); // Note: unused for now to match mod.rs logic
    let mut check_constraints = Vec::new();

    for col in columns {
        let name = col.name.to_string().trim_matches('"').to_string();
        let data_type = col.data_type.to_string();
        let mut is_nullable = true;
        let mut is_primary_key = false;
        let is_unique = false; // We handle unique via table constraints or options later
        let mut column_default = None;
        let mut is_identity = false;
        let mut identity_generation = None;
        let mut collation = None;

        for option in &col.options {
            match &option.option {
                ColumnOption::NotNull => is_nullable = false,
                ColumnOption::Unique(_) => {
                    // Handle unique if needed, currently we check is_unique later or via table constraints
                }
                ColumnOption::PrimaryKey(_) => {
                    is_primary_key = true;
                }
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
                _ => {}
            }
        }

        // Check table constraints for PK
        for constraint in constraints {
            match constraint {
                TableConstraint::PrimaryKey(pk) => {
                    if pk.columns.iter().any(|c| strip_quotes(&c.to_string()) == name) {
                        is_primary_key = true;
                    }
                }
                TableConstraint::Unique(_) => {
                    // Handled via indexes/constraints
                }
                _ => {}
            }
        }

        // Primary key columns are implicitly NOT NULL in PostgreSQL
        if is_primary_key {
            is_nullable = false;
        }

        infos.insert(
            name.clone(),
            ColumnInfo {
                column_name: name,
                data_type: data_type.clone(),
                is_nullable,
                is_primary_key,
                is_unique,
                column_default,
                is_identity,
                identity_generation,
                collation,
                udt_name: data_type,
                enum_name: None,
                is_array: false,
                comment: None,
            },
        );
    }

    (infos, fks, vec![], check_constraints)
}
