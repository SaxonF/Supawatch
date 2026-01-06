pub fn normalize_sql(sql: &str) -> String {
    // First collapse whitespace
    let collapsed: String = sql.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    
    // Remove spaces around parentheses and brackets for consistent comparison
    // This handles differences like "any (array[" vs "any(array["
    collapsed
        .replace(" (", "(")
        .replace("( ", "(")
        .replace(" )", ")")
        .replace(") ", ")")
        .replace(" [", "[")
        .replace("[ ", "[")
        .replace(" ]", "]")
        .replace("] ", "]")
}

pub fn normalize_option(opt: &Option<String>) -> Option<String> {
    opt.as_ref().map(|s| normalize_sql(s))
}
