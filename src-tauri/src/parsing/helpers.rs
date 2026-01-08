use sqlparser::ast::ObjectName;

/// Strip surrounding double-quotes from an identifier.
/// PostgreSQL identifiers can be quoted like "my_column" or bare like my_column.
/// This ensures consistency with introspection which returns bare names.
pub fn strip_quotes(s: &str) -> String {
    s.trim_matches('"').to_string()
}

pub fn parse_object_name(name: &ObjectName) -> (String, String) {
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
