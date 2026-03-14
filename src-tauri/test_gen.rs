fn main() {
    let mut func = FunctionInfo {
        schema: "public".into(),
        name: "test_func".into(),
        args: vec![],
        return_type: "trigger".into(),
        language: "plpgsql".into(),
        definition: "BEGIN\n  return NEW;\nEND;".into(),
        volatility: None,
        is_strict: false,
        security_definer: true,
        config_params: vec![],
        grants: vec![],
        extension: None,
    };
    
    println!("Generated: {}", generate_create_function(&func));
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub schema: String,
    pub name: String,
    pub args: Vec<String>, // simplified
    pub return_type: String,
    pub language: String,
    pub definition: String,
    pub volatility: Option<String>,
    pub is_strict: bool,
    pub security_definer: bool,
    pub config_params: Vec<(String, String)>,
    pub grants: Vec<String>,
    pub extension: Option<String>,
}

pub fn generate_create_function(func: &FunctionInfo) -> String {
    if func.definition.trim_start().to_uppercase().starts_with("CREATE") {
        return format!("{};", func.definition.trim_end_matches(';'));
    }

    let mut sql = format!(
        "CREATE OR REPLACE FUNCTION \"{}\".\"{}\"() RETURNS {} LANGUAGE {} ",
        func.schema, func.name, 
        func.return_type,
        func.language
    );

    if let Some(volatility) = &func.volatility {
        sql.push_str(&format!("{} ", volatility));
    }

    if func.is_strict {
        sql.push_str("STRICT ");
    }

    if func.security_definer {
        sql.push_str("SECURITY DEFINER ");
    }

    for (param, value) in &func.config_params {
        sql.push_str(&format!("SET {} = '{}' ", param, value));
    }

    sql.push_str(&format!("AS $${}$$;", func.definition));

    sql
}
