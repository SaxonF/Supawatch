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

pub fn format_check_expression(expr_str: String) -> String {
    let trimmed = expr_str.trim();
    let upper = trimmed.to_uppercase();
    if upper.starts_with("CHECK ") || upper.starts_with("CHECK(") {
        trimmed.to_string()
    } else {
        format!("CHECK ({})", trimmed)
    }
}


pub fn normalize_data_type(data_type: &str) -> String {
    let lower = data_type.to_lowercase();
    let trimmed = lower.trim();

    // Strip schema prefixes from types
    let known_schema_prefixes = [
        "public.", "extensions.", "pg_catalog.",
    ];
    let trimmed = known_schema_prefixes.iter()
        .find(|prefix| trimmed.starts_with(*prefix))
        .map(|prefix| &trimmed[prefix.len()..])
        .unwrap_or(trimmed);

    // Check for exact matches first
    match trimmed {
        "decimal" => "numeric".to_string(),
        "int" | "int4" | "serial" => "integer".to_string(), // Normalize serial to integer as well (for types not defaults)
        "int8" | "bigserial" => "bigint".to_string(),
        "int2" | "smallserial" => "smallint".to_string(),
        "bool" => "boolean".to_string(),
        "float8" | "float" => "double precision".to_string(), // In Postgres 'float' is double per default
        "real" | "float4" => "real".to_string(),
        "character varying" | "varchar" => "text".to_string(), // Often normalized to text by Supabase usage
        // Note: strictly varchar != text in PG, but for diffing purposes we might want to align if user uses one and DB uses other?
        // Actually, let's stick to strict type if possible, but many users interchange them.
        // For now, let's keep it safe. But `text` is standard in Supabase usually.
        // "varchar" => "character varying".to_string(),
        
        // Handle array types recursively
        s if s.ends_with("[]") => {
            let inner = &s[..s.len() - 2];
            format!("{}[]", normalize_data_type(inner))
        },
        _ => trimmed.to_string(),
    }
}
