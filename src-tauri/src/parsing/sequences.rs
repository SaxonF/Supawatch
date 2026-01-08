use crate::schema::SequenceInfo;
use sqlparser::ast::SequenceOptions;
use std::collections::HashMap;
use super::helpers::parse_object_name;

pub fn handle_create_sequence(
    sequences: &mut HashMap<String, SequenceInfo>,
    stmt_name: sqlparser::ast::ObjectName,
    data_type: Option<sqlparser::ast::DataType>,
    sequence_options: Vec<SequenceOptions>,
) {
    let (schema, seq_name) = parse_object_name(&stmt_name);
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
            SequenceOptions::StartWith(v, _) => {
                start_value = v.to_string().parse().unwrap_or(1);
            }
            SequenceOptions::MinValue(Some(v)) => {
                min_value = v.to_string().parse().unwrap_or(1);
            }
            SequenceOptions::MaxValue(Some(v)) => {
                max_value = v.to_string().parse().unwrap_or(i64::MAX);
            }
            SequenceOptions::IncrementBy(v, _) => {
                increment = v.to_string().parse().unwrap_or(1);
            }
            SequenceOptions::Cycle(c) => cycle = c,
            SequenceOptions::Cache(v) => {
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
