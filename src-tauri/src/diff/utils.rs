pub fn normalize_sql(sql: &str) -> String {
    // First collapse whitespace
    let collapsed: String = sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    
    // Remove spaces around parentheses and brackets for consistent comparison
    // This handles differences like "any (array[" vs "any(array["
    let mut normalized = collapsed
        .replace(" (", "(")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(") ", ")")
        .replace(" [", "[")
        .replace("[ ", "[")
        .replace(" ]", "]")
        .replace("] ", "]");
    
    // Strip outer wrapping parentheses if they wrap the entire expression
    // This handles cases like "(auth.uid() = user_id)" vs "auth.uid() = user_id"
    while normalized.starts_with('(') && normalized.ends_with(')') {
        // Check if these parens actually wrap the whole expression
        let inner = &normalized[1..normalized.len()-1];
        // Verify paren balance - if balanced, the outer parens are just wrappers
        let mut depth = 0;
        let mut balanced = true;
        for c in inner.chars() {
            match c {
                '(' => depth += 1,
                ')' => {
                    if depth == 0 {
                        // Found unmatched close paren, so outer parens are not just wrappers
                        balanced = false;
                        break;
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
        if balanced && depth == 0 {
            // Safe to strip outer parens
            normalized = inner.to_string();
        } else {
            break;
        }
    }
    
    normalized
}

pub fn normalize_option(opt: &Option<String>) -> Option<String> {
    opt.as_ref().map(|s| normalize_sql(s))
}

/// Normalize function definitions for comparison.
/// Handles differences between remote introspection and local parsing:
/// - Dollar quoting: $function$...$function$ vs $$...$$
/// - Quoted identifiers: "public"."func_name" vs public.func_name
/// - Whitespace normalization
/// - Case normalization for language keywords
pub fn normalize_function_definition(definition: &str) -> String {
    let mut s = definition.to_string();
    
    // Normalize dollar quoting - replace common $<tag>$ patterns with $$
    // These are the most common dollar-quote tags used in PostgreSQL
    let dollar_quote_tags = [
        "$function$", "$FUNCTION$", 
        "$body$", "$BODY$",
        "$code$", "$CODE$",
        "$sql$", "$SQL$",
        "$plpgsql$", "$PLPGSQL$",
    ];
    for tag in dollar_quote_tags {
        s = s.replace(tag, "$$");
    }
    
    // Remove double quotes around identifiers
    // This handles "public"."func_name" -> public.func_name
    s = s.replace("\"", "");
    
    // Apply standard SQL normalization (collapses whitespace, lowercases, normalizes parens)
    normalize_sql(&s)
}

/// Normalize view definitions for comparison.
/// Handles differences between remote introspection (pg_get_viewdef) and local parsing:
/// - Local includes full "CREATE OR REPLACE VIEW ... AS SELECT ..." 
/// - Remote returns just the "SELECT ..." part
/// - Quoted identifiers, whitespace, type casts
/// - pg_get_viewdef adds extra parens in FILTER(WHERE(...)) vs FILTER(WHERE ...)
/// - pg_get_viewdef adds nested parens around JOINs: FROM((t1 join t2...
pub fn normalize_view_definition(definition: &str) -> String {
    let mut s = definition.to_string();
    
    // Strip CREATE [OR REPLACE] VIEW ... AS prefix to get just the SELECT statement
    // Local parsing includes the full statement, remote introspection only returns the query
    let lower = s.to_lowercase();
    if let Some(as_pos) = lower.find(" as ") {
        // Check if this looks like a CREATE VIEW statement (starts with CREATE)
        let trimmed_lower = lower.trim_start();
        if trimmed_lower.starts_with("create") {
            // Skip past the " AS " to get just the SELECT statement
            s = s[as_pos + 4..].to_string();
        }
    }
    
    // Remove double quotes around identifiers
    s = s.replace("\"", "");
    
    // Apply standard SQL normalization (collapses whitespace, lowercases, normalizes parens)
    let mut normalized = normalize_sql(&s);
    
    // Iteratively collapse nested parentheses (( -> ( and )) -> )
    // pg_get_viewdef wraps JOINs and other constructs in extra parens
    // Do this FIRST before other replacements
    loop {
        let before = normalized.clone();
        normalized = normalized.replace("((", "(");
        normalized = normalized.replace("))", ")");
        if normalized == before {
            break;
        }
    }
    
    // Handle pg_get_viewdef adding extra parentheses in FILTER(WHERE(...))
    // Normalize "filter(where(" to "filter(where "
    normalized = normalized.replace("filter(where(", "filter(where ");
    
    // Handle pg_get_viewdef wrapping FROM clause in parentheses: FROM(table vs FROM table
    normalized = normalized.replace("from(", "from ");
    normalized = normalized.replace("from (", "from ");
    
    // Handle pg_get_viewdef wrapping ON clause conditions in parentheses: ON(condition) vs ON condition
    normalized = normalized.replace("on(", "on ");
    normalized = normalized.replace("on (", "on ");
    
    // Remove orphaned closing parens before SQL keywords that might result from the above
    // These patterns occur when we remove opening parens but the closing ones remain
    normalized = normalized.replace(")left", " left");
    normalized = normalized.replace(") left", " left");
    normalized = normalized.replace(")right", " right");
    normalized = normalized.replace(") right", " right");
    normalized = normalized.replace(")inner", " inner");
    normalized = normalized.replace(") inner", " inner");
    normalized = normalized.replace(")join", " join");
    normalized = normalized.replace(") join", " join");
    normalized = normalized.replace(")group", " group");
    normalized = normalized.replace(") group", " group");
    normalized = normalized.replace(")order", " order");
    normalized = normalized.replace(") order", " order");
    normalized = normalized.replace(")where", " where");
    normalized = normalized.replace(") where", " where");
    
    // Strip trailing semicolon - pg_get_viewdef includes it, sqlparser doesn't
    let normalized = normalized.trim_end_matches(';').to_string();
    
    normalized
}

/// Normalize CHECK constraint expressions for comparison.
/// Strips the CHECK keyword, type casts, and normalizes expressions.
/// This handles differences between local parsing (CHECK (status IN ('a', 'b'))) and
/// remote introspection (CHECK (((status)::text = ANY ((ARRAY['a'::text, 'b'::text])::text[])))).
pub fn normalize_check_expression(expr: &str) -> String {
    let mut s = expr.trim().to_lowercase();
    
    // Remove leading CHECK keyword if present
    if s.starts_with("check") {
        s = s.trim_start_matches("check").trim().to_string();
    }
    
    // Strip type casts like ::text, ::text[], ::integer, etc.
    // This regex removes PostgreSQL type cast syntax
    let type_cast_suffixes = [
        "::text[]", "::text", "::integer[]", "::integer", "::int[]", "::int",
        "::character varying[]", "::character varying", "::varchar[]", "::varchar",
        "::boolean", "::bool", "::numeric", "::jsonb[]", "::jsonb", "::uuid[]", "::uuid",
    ];
    
    for suffix in type_cast_suffixes {
        s = s.replace(suffix, "");
    }
    
    // Then apply normal SQL normalization (handles parens, whitespace, etc.)
    normalize_sql(&s)
}

/// Normalize default value expressions for comparison.
/// Strips type casts like ::text, ::integer, etc. and normalizes quotes.
/// This handles differences between local parsing ('value') and
/// remote introspection ('value'::text).
pub fn normalize_default_value(expr: &str) -> String {
    let mut s = expr.trim().to_lowercase();
    
    // Strip common type casts at the end (::text, ::integer, etc.)
    // Handle patterns like 'value'::text or 'value'::character varying
    let type_cast_patterns = [
        "::text",
        "::integer",
        "::int",
        "::bigint",
        "::smallint",
        "::boolean",
        "::bool",
        "::numeric",
        "::real",
        "::double precision",
        "::character varying",
        "::varchar",
        "::uuid",
        "::timestamp with time zone",
        "::timestamp without time zone",
        "::timestamptz",
        "::timestamp",
        "::date",
        "::time",
        "::jsonb",
        "::json",
    ];
    
    for pattern in type_cast_patterns {
        if s.ends_with(pattern) {
            s = s[..s.len() - pattern.len()].to_string();
            break;
        }
    }
    
    // Also handle type cast with any type by matching ::
    // Only strip if it's a type cast after a quoted string or simple value
    if let Some(idx) = s.rfind("::") {
        let before = &s[..idx];
        // Only strip if what's before looks like a value (ends with ' or is alphanumeric/parentheses)
        if before.ends_with('\'') || before.ends_with(')') {
            s = before.to_string();
        }
    }
    
    // Apply normal normalization
    normalize_sql(&s)
}

/// Helper to normalize Option<String> default values
pub fn normalize_default_option(opt: &Option<String>) -> Option<String> {
    opt.as_ref().map(|s| normalize_default_value(s))
}

