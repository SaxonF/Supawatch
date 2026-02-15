//! Helper functions for parsing PostgreSQL introspection results.

use crate::schema::FunctionArg;
use serde::Deserialize;

/// Parse a PostgreSQL array value from JSON.
pub fn parse_pg_array(val: &serde_json::Value) -> Vec<String> {
    if let Some(arr) = val.as_array() {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    } else if let Some(s) = val.as_str() {
        // Handle "{a,b}" string
        s.trim_matches(|c| c == '{' || c == '}')
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        vec![]
    }
}



/// Parse policy command character to SQL command name.
pub fn parse_policy_cmd(cmd: &str) -> String {
    match cmd {
        "r" => "SELECT".to_string(),
        "a" => "INSERT".to_string(),
        "w" => "UPDATE".to_string(),
        "d" => "DELETE".to_string(),
        "*" => "ALL".to_string(),
        _ => cmd.to_string(),
    }
}

/// Parse function arguments string from pg_proc.
pub fn parse_function_args(args_str: &str) -> Vec<FunctionArg> {
    if args_str.is_empty() {
        return vec![];
    }
    args_str
        .split(',')
        .map(|s| {
            let trimmed = s.trim();
            let parts: Vec<&str> = trimmed.splitn(2, ' ').collect();

            // Check for mode keywords
            let (mode, name_type) = if parts.first().map(|p| p.to_uppercase()).as_deref()
                == Some("IN")
                || parts.first().map(|p| p.to_uppercase()).as_deref() == Some("OUT")
                || parts.first().map(|p| p.to_uppercase()).as_deref() == Some("INOUT")
                || parts.first().map(|p| p.to_uppercase()).as_deref() == Some("VARIADIC")
            {
                (
                    Some(parts[0].to_uppercase()),
                    parts.get(1).map(|s| *s).unwrap_or(""),
                )
            } else {
                (None, trimmed)
            };

            let name_type_parts: Vec<&str> = name_type.splitn(2, ' ').collect();
            let (name, type_str) = if name_type_parts.len() >= 2 {
                (
                    name_type_parts[0].trim_matches('"').to_string(),
                    name_type_parts[1..].join(" "),
                )
            } else {
                (String::new(), name_type.to_string())
            };

            // Check for DEFAULT
            let (final_type, default_value) =
                if let Some(idx) = type_str.to_uppercase().find(" DEFAULT ") {
                    (
                        type_str[..idx].to_string(),
                        Some(type_str[idx + 9..].to_string()),
                    )
                } else {
                    (type_str, None)
                };

            FunctionArg {
                name,
                type_: final_type,
                mode,
                default_value,
            }
        })
        .collect()
}

/// Extract WHEN clause from trigger definition.
pub fn extract_trigger_when_clause(trigger_def: &str) -> Option<String> {
    let upper = trigger_def.to_uppercase();
    if let Some(when_idx) = upper.find(" WHEN ") {
        let after_when = &trigger_def[when_idx + 6..];
        if let Some(start) = after_when.find('(') {
            let mut depth = 0;
            let mut end = None;
            for (i, c) in after_when[start..].char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(start + i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(e) = end {
                return Some(after_when[start + 1..e].to_string());
            }
        }
    }
    None
}

/// Extract UPDATE OF columns from trigger definition if present.
/// Returns Some(vec![col1, col2]) for "UPDATE OF col1, col2" or None for plain "UPDATE".
pub fn extract_update_of_columns(trigger_def: &str) -> Option<Vec<String>> {
    let upper = trigger_def.to_uppercase();
    
    // Look for "UPDATE OF " pattern
    if let Some(update_of_idx) = upper.find("UPDATE OF ") {
        let after_update_of = &trigger_def[update_of_idx + 10..]; // Skip "UPDATE OF "
        
        // Find where the columns end - before " ON " which follows the column list
        let end_idx = upper[update_of_idx + 10..].find(" ON ")
            .unwrap_or(after_update_of.len());
        
        let columns_str = &after_update_of[..end_idx];
        
        // Parse comma-separated column names, handling quoted identifiers
        let columns: Vec<String> = columns_str
            .split(',')
            .map(|s| {
                let trimmed = s.trim();
                // Strip quotes if present
                trimmed.trim_matches('"').to_string()
            })
            .filter(|s| !s.is_empty())
            .collect();
        
        if !columns.is_empty() {
            return Some(columns);
        }
    }
    
    None
}

/// Extract expressions from index definition.
/// Handles cases like: CREATE INDEX idx ON table (col, lower(name)) WHERE condition
pub fn extract_index_expressions(index_def: &str) -> Vec<String> {
    let mut expressions = vec![];

    if let Some(on_idx) = index_def.to_uppercase().find(" ON ") {
        let after_on = &index_def[on_idx + 4..];
        if let Some(paren_start) = after_on.find('(') {
            // Find the matching closing paren by tracking depth
            // This avoids capturing content from WHERE clauses
            let in_parens = &after_on[paren_start + 1..];
            let mut depth = 1;
            let mut paren_end = None;
            for (i, c) in in_parens.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            paren_end = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(end) = paren_end {
                let cols_str = &in_parens[..end];
                for part in cols_str.split(',') {
                    let trimmed = part.trim();
                    if trimmed.contains('(') {
                        expressions.push(trimmed.to_string());
                    }
                }
            }
        }
    }

    expressions
}

/// Custom deserializer for i64 that handles string or int.
pub fn deserialize_i64_or_string<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StrOrInt {
        Str(String),
        Int(i64),
    }

    match StrOrInt::deserialize(deserializer)? {
        StrOrInt::Str(v) => v.parse::<i64>().map_err(serde::de::Error::custom),
        StrOrInt::Int(v) => Ok(v),
    }
}
