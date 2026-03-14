fn main() {
    let sql = r#"
create or replace function public.sync_agent_task_cron()
returns trigger
language plpgsql
security definer
as $$
declare
  v_job_name text;
begin
  return NEW;
end;
$$;
"#;
    let (cleaned, opts) = preprocess_function_options(sql);
    println!("Cleaned:\n{}", cleaned);
    println!("Opts: {:?}", opts);
}

// Result of preprocessing function options from SQL
// Contains the cleaned SQL and maps of function names to their extracted options
#[derive(Debug)]
struct FunctionOptions {
    security_definer: bool,
    config_params: Vec<(String, String)>,
}

fn preprocess_function_options(sql: &str) -> (String, std::collections::HashMap<String, FunctionOptions>) {
    let mut func_options: std::collections::HashMap<String, FunctionOptions> = std::collections::HashMap::new();
    
    // We will collect ranges to remove from the SQL string
    // Store as (start, end)
    let mut removal_ranges: Vec<(usize, usize)> = vec![];
    
    let sql_upper = sql.to_uppercase();
    
    // Find all functions and their positions
    let mut func_positions: Vec<(usize, String)> = vec![];
    let mut start_search = 0;
    
    while let Some(func_idx) = sql_upper[start_search..].find("FUNCTION") {
        let abs_idx = start_search + func_idx;
        // Find the opening parenthesis after FUNCTION to isolate name
        if let Some(paren_idx) = sql[abs_idx..].find('(') {
            let raw_name = sql[abs_idx + 8..abs_idx + paren_idx].trim().to_string();
            func_positions.push((abs_idx, raw_name));
        }
        start_search = abs_idx + 8;
    }
    
    // For each function, look for SECURITY DEFINER and SET clauses
    for (i, (func_pos, func_name)) in func_positions.iter().enumerate() {
        let mut options = FunctionOptions {
            security_definer: false,
            config_params: vec![],
        };
        
        // Find the end of this function definition to limit scope
        let search_end = if i + 1 < func_positions.len() {
            func_positions[i + 1].0
        } else {
            sql.len()
        };
        
        // We need to find where the function body starts (AS $...$ or AS '...')
        // We only scan for options BEFORE the body start
        let func_slice = &sql[*func_pos..search_end];
        let func_slice_upper = func_slice.to_uppercase();
        
        // Try to find " AS " which introduces the body
        // This is heuristic but covers standard CREATE FUNCTION syntax
        // strict/immutable/etc attributes come before AS
        // Try to find " AS " which introduces the body using Regex to handle newlines
        // Pattern matches whitespace or word boundary before AS, and whitespace after
        let as_regex = regex::Regex::new(r"(?i)\s+AS\s+").unwrap();
        let body_start_offset = if let Some(mat) = as_regex.find(func_slice) {
            mat.start()
        } else {
             // Fallback
             func_slice.len()
        };
        
        let header_slice = &func_slice[..body_start_offset];
        let header_slice_upper = header_slice.to_uppercase();
        
        // Check for SECURITY DEFINER in header
        if let Some(sd_idx) = header_slice_upper.find("SECURITY DEFINER") {
             options.security_definer = true;
             // Schedule removal
             let start = *func_pos + sd_idx;
             let end = start + "SECURITY DEFINER".len();
             removal_ranges.push((start, end));
        }
        
        // Look for SET clauses in header
        // Pattern: SET param_name = 'value' or SET param_name TO value
        let set_regex = regex::Regex::new(r"(?i)\bSET\s+(\w+)\s*=\s*'([^']*)'").unwrap();
        for cap in set_regex.captures_iter(header_slice) {
            let param_name = cap.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
            let param_value = cap.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
            options.config_params.push((param_name, param_value));
            
            // Schedule removal of the entire match
            let match_range = cap.get(0).unwrap().range();
            let start = *func_pos + match_range.start;
            let end = *func_pos + match_range.end;
            removal_ranges.push((start, end));
        }
        
        func_options.insert(func_name.clone(), options);
    }
    
    // Sort ranges by start position (descending) to remove safely
    removal_ranges.sort_by(|a, b| b.0.cmp(&a.0));
    
    let mut cleaned_sql = sql.to_string();
    for (start, end) in removal_ranges {
        if start < cleaned_sql.len() && end <= cleaned_sql.len() {
             // Replace with spaces to preserve line numbers/positions
             let length = end - start;
             let spaces = " ".repeat(length);
             cleaned_sql.replace_range(start..end, &spaces);
        }
    }
    
    (cleaned_sql, func_options)
}
