use crate::schema::{PolicyInfo, TableInfo, TriggerInfo};
use sqlparser::ast::{CreatePolicyCommand, CreateTrigger, TriggerExecBody};
use std::collections::HashMap;
use super::helpers::{parse_object_name, strip_quotes};

pub fn handle_create_trigger(
    tables: &mut HashMap<String, TableInfo>,
    stmt: CreateTrigger,
) {
    let CreateTrigger {
        name,
        table_name,
        period,
        events,
        exec_body,
        trigger_object,
        condition,
        ..
    } = stmt;

    let t_name = strip_quotes(&name.to_string());
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
        strip_quotes(&func_desc.name.to_string())
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

pub fn handle_create_policy(
    tables: &mut HashMap<String, TableInfo>,
    name: sqlparser::ast::Ident,
    table_name: sqlparser::ast::ObjectName,
    command: Option<CreatePolicyCommand>,
    to: Option<Vec<sqlparser::ast::Owner>>,
    using: Option<sqlparser::ast::Expr>,
    with_check: Option<sqlparser::ast::Expr>,
) {
    let p_name = name.value;
    let (t_schema, t_table) = parse_object_name(&table_name);
    let table_key = format!("\"{}\".\"{}\"", t_schema, t_table);

    let cmd = match command {
        Some(CreatePolicyCommand::All) | None => "ALL",
        Some(CreatePolicyCommand::Select) => "SELECT",
        Some(CreatePolicyCommand::Insert) => "INSERT",
        Some(CreatePolicyCommand::Update) => "UPDATE",
        Some(CreatePolicyCommand::Delete) => "DELETE",
    }
    .to_string();

    let roles_vec = if let Some(r_vec) = to {
        r_vec.iter().map(|owner| owner.to_string()).collect()
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
