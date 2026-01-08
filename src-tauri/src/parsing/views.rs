use crate::schema::ViewInfo;
use sqlparser::ast::{CreateTableOptions, CreateView};
use std::collections::HashMap;
use super::helpers::parse_object_name;

pub fn handle_create_view(
    views: &mut HashMap<String, ViewInfo>,
    stmt: CreateView,
) {
    let CreateView {
        name,
        query,
        materialized,
        options,
        ..
    } = stmt;

    let (schema, view_name) = parse_object_name(&name);
    let definition = query.to_string();

    let with_options: Vec<String> = match options {
        CreateTableOptions::Options(opts) => opts.iter().map(|o| o.to_string()).collect(),
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
