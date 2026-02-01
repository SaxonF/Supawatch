use crate::schema::{FunctionGrant, FunctionInfo, SequenceInfo, ViewInfo};

pub fn ensure_quoted(name: &str) -> String {
    if name.starts_with('"') && name.ends_with('"') {
        name.to_string()
    } else {
        format!("\"{}\"", name)
    }
}





pub fn generate_create_sequence(seq: &SequenceInfo) -> String {
    let mut sql = format!("CREATE SEQUENCE \"{}\".\"{}\"", seq.schema, seq.name);

    if seq.data_type != "bigint" {
        sql.push_str(&format!(" AS {}", seq.data_type));
    }

    sql.push_str(&format!(" START WITH {}", seq.start_value));
    sql.push_str(&format!(" INCREMENT BY {}", seq.increment));
    sql.push_str(&format!(" MINVALUE {}", seq.min_value));
    sql.push_str(&format!(" MAXVALUE {}", seq.max_value));
    sql.push_str(&format!(" CACHE {}", seq.cache_size));

    if seq.cycle {
        sql.push_str(" CYCLE");
    } else {
        sql.push_str(" NO CYCLE");
    }

    if let Some(owned_by) = &seq.owned_by {
        sql.push_str(&format!(" OWNED BY {}", owned_by));
    }

    sql.push(';');
    sql
}

pub fn generate_alter_sequence(seq: &SequenceInfo) -> String {
    let mut parts = vec![];

    parts.push(format!("INCREMENT BY {}", seq.increment));
    parts.push(format!("MINVALUE {}", seq.min_value));
    parts.push(format!("MAXVALUE {}", seq.max_value));
    parts.push(format!("CACHE {}", seq.cache_size));

    if seq.cycle {
        parts.push("CYCLE".to_string());
    } else {
        parts.push("NO CYCLE".to_string());
    }

    format!("ALTER SEQUENCE \"{}\".\"{}\" {};", seq.schema, seq.name, parts.join(" "))
}

pub fn generate_create_function(func: &FunctionInfo) -> String {
    // If definition starts with CREATE OR REPLACE, use it directly (from introspection)
    // Otherwise construct it (from local parsing)
    if func.definition.trim_start().to_uppercase().starts_with("CREATE") {
        return format!("{};", func.definition.trim_end_matches(';'));
    }

    let mut sql = format!(
        "CREATE OR REPLACE FUNCTION \"{}\".\"{}\"({}) RETURNS {} LANGUAGE {} ",
        func.schema, func.name, 
        func.args.iter().map(|a| {
            let arg_name = a.name.trim_matches('"');
            let mut arg_def = format!("\"{}\" {}", arg_name, a.type_);
            if let Some(mode) = &a.mode {
                arg_def = format!("{} {}", mode, arg_def);
            }
            if let Some(default) = &a.default_value {
                arg_def.push_str(&format!(" DEFAULT {}", default));
            }
            arg_def
        }).collect::<Vec<_>>().join(", "),
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

pub fn generate_create_view(view: &ViewInfo) -> String {
    let mut sql = String::new();

    if view.is_materialized {
        sql.push_str(&format!("CREATE MATERIALIZED VIEW \"{}\".\"{}\"", view.schema, view.name));
    } else {
        sql.push_str(&format!("CREATE OR REPLACE VIEW \"{}\".\"{}\"", view.schema, view.name));
    }

    if !view.with_options.is_empty() {
        sql.push_str(&format!(" WITH ({})", view.with_options.join(", ")));
    }

    sql.push_str(&format!(" AS {}", view.definition));

    if let Some(check) = &view.check_option {
        sql.push_str(&format!(" WITH {} CHECK OPTION", check));
    }

    if !sql.ends_with(';') {
        sql.push(';');
    }

    sql
}

/// Generate GRANT EXECUTE statements for a function
pub fn generate_function_grants(func: &FunctionInfo) -> Vec<String> {
    func.grants.iter().map(|grant| {
        let arg_types: Vec<String> = func.args.iter().map(|a| a.type_.clone()).collect();
        format!(
            "GRANT {} ON FUNCTION \"{}\".\"{}\"{} TO \"{}\";",
            grant.privilege,
            func.schema,
            func.name,
            format!("({})", arg_types.join(", ")),
            grant.grantee
        )
    }).collect()
}
