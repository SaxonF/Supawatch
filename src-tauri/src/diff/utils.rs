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
