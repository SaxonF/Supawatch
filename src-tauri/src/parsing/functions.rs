use crate::schema::{FunctionArg, FunctionInfo};
use sqlparser::ast::{CreateFunction, CreateFunctionBody, Expr, OperateFunctionArg, Value};
use std::collections::HashMap;
use super::helpers::{parse_object_name, strip_quotes};

pub fn handle_create_function(
    functions: &mut HashMap<String, FunctionInfo>,
    stmt: CreateFunction,
    security_definer: bool,
) {
    let CreateFunction {
        name,
        args,
        return_type,
        language,
        function_body,
        behavior,
        called_on_null,
        ..
    } = stmt;

    let (schema, fn_name) = parse_object_name(&name);
    let ret_type = return_type
        .map(|t| t.to_string().to_lowercase())
        .unwrap_or("void".to_string());

    let mut fn_args = vec![];
    if let Some(arg_list) = args {
        for arg in arg_list {
            let OperateFunctionArg {
                name: arg_name,
                data_type,
                default_expr,
                mode,
            } = arg;

            let type_str = data_type.to_string().to_lowercase();
            fn_args.push(FunctionArg {
                name: arg_name.map(|n| strip_quotes(&n.value)).unwrap_or_default(),
                type_: type_str,
                mode: mode.map(|m| m.to_string()),
                default_value: default_expr.map(|d| d.to_string()),
            });
        }
    }

    let lang = language.map(|l| l.value).unwrap_or("sql".to_string());
    let volatility = behavior.map(|b| b.to_string());
    let is_strict = called_on_null
        .map(|c| c.to_string().contains("STRICT"))
        .unwrap_or(false);

    let def = if let Some(CreateFunctionBody::AsBeforeOptions { body, .. }) = function_body {
        match body {
            Expr::Value(v) => match v.value {
                Value::DollarQuotedString(d) => d.value.trim().to_string(),
                Value::SingleQuotedString(s) => s.trim().to_string(),
                _ => v.to_string().trim().to_string(),
            },
            _ => body.to_string(),
        }
    } else {
        "".to_string()
    };

    let arg_types: Vec<String> = fn_args.iter().map(|a| a.type_.clone()).collect();
    let signature = format!("\"{}\".\"{}\"({})", schema, fn_name, arg_types.join(", "));

    functions.insert(
        signature,
        FunctionInfo {
            schema,
            name: fn_name,
            args: fn_args,
            return_type: ret_type,
            language: lang,
            definition: def,
            volatility,
            is_strict,
            security_definer,
        },
    );
}


#[cfg(test)]
mod tests {
    use super::*;
    use sqlparser::dialect::PostgreSqlDialect;
    use sqlparser::parser::Parser;

    #[test]
    fn test_parse_create_function_quoted_args() {
        let sql = r#"CREATE OR REPLACE FUNCTION "public"."generate_world"("seed" integer DEFAULT 0) RETURNS void LANGUAGE plpgsql VOLATILE AS $$BEGIN END;$$;"#;
        let dialect = PostgreSqlDialect {};
        let mut ast = Parser::parse_sql(&dialect, sql).unwrap();
        let stmt = ast.pop().unwrap();

        match stmt {
            sqlparser::ast::Statement::CreateFunction(stmt) => {
                let mut functions = HashMap::new();
                handle_create_function(&mut functions, stmt, false);

                let func = functions.values().next().unwrap();
                let arg = &func.args[0];
                assert_eq!(arg.name, "seed"); // Should be unquoted
            }
            _ => panic!("Expected CreateFunction"),
        }
    }
}
