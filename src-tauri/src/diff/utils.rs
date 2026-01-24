use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sqlparser::ast::{Expr, Statement, Value, SetExpr, TableFactor, SelectItem};


fn clean_function_arg(arg: sqlparser::ast::FunctionArg) -> sqlparser::ast::FunctionArg {
    match arg {
        sqlparser::ast::FunctionArg::Named { name, arg, operator } => sqlparser::ast::FunctionArg::Named { 
            name, 
            operator,
            arg: match arg {
                sqlparser::ast::FunctionArgExpr::Expr(e) => sqlparser::ast::FunctionArgExpr::Expr(clean_expr(e)),
                _ => arg,
            }
        },
        sqlparser::ast::FunctionArg::Unnamed(arg_expr) => {
            match arg_expr {
                sqlparser::ast::FunctionArgExpr::Expr(e) => sqlparser::ast::FunctionArg::Unnamed(sqlparser::ast::FunctionArgExpr::Expr(clean_expr(e))),
                _ => sqlparser::ast::FunctionArg::Unnamed(arg_expr),
            }
        },
        sqlparser::ast::FunctionArg::ExprNamed { name, arg, operator } => sqlparser::ast::FunctionArg::ExprNamed {
            name,
            operator,
            arg: match arg {
                sqlparser::ast::FunctionArgExpr::Expr(e) => sqlparser::ast::FunctionArgExpr::Expr(clean_expr(e)),
                _ => arg,
            }
        },
    }
}

fn clean_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Nested(inner) => clean_expr(*inner),
        Expr::Cast { expr: inner, .. } => clean_expr(*inner),
        // Handle interval '...' which parses as Expr::Interval in newer sqlparser
        Expr::Interval(interval) => clean_expr(*interval.value),
        
        // Recurse common structures
        Expr::BinaryOp { left, op, right } => Expr::BinaryOp {
            left: Box::new(clean_expr(*left)),
            op,
            right: Box::new(clean_expr(*right)),
        },
        Expr::UnaryOp { op, expr } => Expr::UnaryOp {
            op,
            expr: Box::new(clean_expr(*expr)),
        },
        Expr::Function(mut func) => {
            match func.args {
                sqlparser::ast::FunctionArguments::List(mut list) => {
                    list.args = list.args.into_iter().map(clean_function_arg).collect();
                    func.args = sqlparser::ast::FunctionArguments::List(list);
                }
                _ => {}
            }
            // Clean FILTER clause (e.g. FILTER (WHERE ...))
            if let Some(filter) = func.filter {
                func.filter = Some(Box::new(clean_expr(*filter)));
            }
            // Clean OVER clause (window function)
            if let Some(window) = func.over {
                // WindowType can be NamedWindow or WindowSpec
                // We only care about WindowSpec
                match window {
                    sqlparser::ast::WindowType::WindowSpec(mut spec) => {
                        spec.partition_by = spec.partition_by.into_iter().map(clean_expr).collect();
                        spec.order_by = spec.order_by.into_iter().map(|mut ob| {
                            ob.expr = clean_expr(ob.expr);
                            ob
                        }).collect();
                        func.over = Some(sqlparser::ast::WindowType::WindowSpec(spec));
                    },
                    _ => { func.over = Some(window); } // keep as is if named
                }
            }
            
            Expr::Function(func)
        },
        // For other cases, return as is (shallow)
        _ => expr,
    }
}

fn clean_join_constraint(constraint: sqlparser::ast::JoinConstraint) -> sqlparser::ast::JoinConstraint {
    match constraint {
        sqlparser::ast::JoinConstraint::On(expr) => sqlparser::ast::JoinConstraint::On(clean_expr(expr)),
        _ => constraint,
    }
}

fn clean_statement(stmt: Statement) -> Statement {
    match stmt {
        Statement::Query(mut query) => {
            // Clean projection (SELECT list)
            if let SetExpr::Select(mut select) = *query.body {
                select.projection = select.projection.into_iter().map(|item| {
                    match item {
                        SelectItem::UnnamedExpr(expr) => SelectItem::UnnamedExpr(clean_expr(expr)),
                        SelectItem::ExprWithAlias { expr, alias } => SelectItem::ExprWithAlias { expr: clean_expr(expr), alias },
                        _ => item,
                    }
                }).collect();
                
                // Clean WHERE clause
                if let Some(selection) = select.selection {
                    select.selection = Some(clean_expr(selection));
                }

                // Clean JOINs in FROM clause
                // select.from is Vec<TableWithJoins>
                select.from = select.from.into_iter().map(|mut table| {
                    table.joins = table.joins.into_iter().map(|mut join| {
                         // JoinOperator in sqlparser usually wraps JoinConstraint for Inner, Left, Right etc.
                         // But Cross, Implicit, etc. don't have constraints.
                         match join.join_operator {
                             sqlparser::ast::JoinOperator::Inner(constraint) => {
                                 join.join_operator = sqlparser::ast::JoinOperator::Inner(clean_join_constraint(constraint));
                             },
                             sqlparser::ast::JoinOperator::LeftOuter(constraint) => {
                                 join.join_operator = sqlparser::ast::JoinOperator::LeftOuter(clean_join_constraint(constraint));
                             },
                             sqlparser::ast::JoinOperator::RightOuter(constraint) => {
                                 join.join_operator = sqlparser::ast::JoinOperator::RightOuter(clean_join_constraint(constraint));
                             },
                             sqlparser::ast::JoinOperator::FullOuter(constraint) => {
                                 join.join_operator = sqlparser::ast::JoinOperator::FullOuter(clean_join_constraint(constraint));
                             },
                             // Handle aliases if they exist (Left/Right/Full without Outer)
                             // Note: Use wildcard if we suspect valid variants but don't know names,
                             // BUT checking docs for 0.60.0 strongly suggests Left/Right/Full might be distinct from LeftOuter/...
                             // However, if compilation fails we'll know.
                             // Given the log said 'Left', and we fell through, 'Left' must be a variant.
                             // We try to match it by name.
                             #[cfg(not(feature = "ignore_unresolved_variants"))] // defensive
                             sqlparser::ast::JoinOperator::Left(constraint) => {
                                 join.join_operator = sqlparser::ast::JoinOperator::Left(clean_join_constraint(constraint));
                             },
                             // sqlparser::ast::JoinOperator::Right(constraint) => {
                             //    join.join_operator = sqlparser::ast::JoinOperator::Right(clean_join_constraint(constraint));
                             // },
                             // sqlparser::ast::JoinOperator::Full(constraint) => {
                             //    join.join_operator = sqlparser::ast::JoinOperator::Full(clean_join_constraint(constraint));
                             // },
                             _ => {
                                 // eprintln!("MISSED JOIN OPERATOR: {:?}", join.join_operator);
                             }
                         }
                         join
                    }).collect();
                    table
                }).collect();
                
                // Put back
                *query.body = SetExpr::Select(select);
            }
            Statement::Query(query)
        }
        _ => stmt,
    }
}

fn normalize_via_ast(sql: &str) -> Option<String> {
    let dialect = PostgreSqlDialect {};
    // Parse
    let mut ast = Parser::parse_sql(&dialect, sql).ok()?;
    
    // We expect a single statement
    if ast.len() != 1 { return None; }
    
    let statement = ast.into_iter().next().unwrap();
    let cleaned = clean_statement(statement);
    
    Some(cleaned.to_string())
}

pub fn normalize_sql(sql: &str) -> String {
    // Remove double quotes around identifiers first
    // This handles "public"."characters" vs public.characters
    let unquoted = sql.replace("\"", "");
    
    // First collapse whitespace
    let collapsed: String = unquoted.split_whitespace()
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
        .replace("] ", "]")
        .replace(", ", ",")
        .replace(" ,", ",");
    
    // Strip type casts like ::text, ::integer, etc.
    // These are added by PostgreSQL during introspection
    // IMPORTANT: Longer matches must come first (::interval before ::int)
    let type_cast_suffixes = [
        "::text[]", "::text", 
        "::integer[]", "::integer", "::interval", "::int[]", "::int",
        "::bigint[]", "::bigint", "::smallint[]", "::smallint",
        "::character varying[]", "::character varying", "::varchar[]", "::varchar",
        "::boolean", "::bool", "::numeric", "::jsonb[]", "::jsonb", "::uuid[]", "::uuid",
        "::float", "::double precision", "::regclass", "::regtype",
        "::date", "::time", "::timestamp", "::timestamptz",
    ];
    for suffix in type_cast_suffixes {
        normalized = normalized.replace(suffix, "");
    }
    
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

/// Normalize policy expressions for comparison.
/// PostgreSQL rewrites policy expressions when stored, adding table prefixes and removing schema prefixes.
/// e.g., "id FROM public.characters WHERE user_id" becomes 
///       "characters.id FROM characters WHERE characters.user_id"
/// This function normalizes both to a comparable form by stripping table prefixes.
pub fn normalize_policy_expression(sql: &str) -> String {
    let normalized = normalize_sql(sql);
    
    // Strip table prefixes from column references
    // Pattern: word.word where first word is a table name
    // We use a simple heuristic: if we see "tablename." before a word, strip the prefix
    // This handles cases like "characters.id" -> "id" and "characters.user_id" -> "user_id"
    use regex::Regex;
    
    // Match pattern: word followed by dot followed by word (table.column pattern)
    // But be careful not to match function calls like auth.uid()
    // We'll strip "tablename." prefix when it's followed by a lowercase identifier
    let re = Regex::new(r"\b([a-z_][a-z0-9_]*)\.([a-z_][a-z0-9_]*)\b").unwrap();
    
    // Replace table.column with just column, but preserve function calls
    let mut result = re.replace_all(&normalized, |caps: &regex::Captures| {
        let prefix = &caps[1];
        let suffix = &caps[2];
        
        // Preserve known function namespaces like auth.uid(), cron.schedule()
        let known_namespaces = ["auth", "cron", "extensions", "net", "pg_", "supabase"];
        if known_namespaces.iter().any(|ns| prefix.starts_with(ns)) {
            // Keep the full reference for known function namespaces
            format!("{}.{}", prefix, suffix)
        } else {
            // Strip the table prefix for column references
            suffix.to_string()
        }
    }).to_string();
    
    // PostgreSQL adds parentheses after WHERE in subqueries
    // Normalize "where(" to "where " to handle this
    result = result.replace("where(", "where ");
    
    // Also handle other keywords that might have extra parens
    result = result.replace("and(", "and ");
    result = result.replace("or(", "or ");
    
    // Now we may have unbalanced parens from the above replacements
    // Strip the corresponding trailing paren if the expression ends with ")"
    // Count parens to find unbalanced trailing ones
    let open_count = result.chars().filter(|c| *c == '(').count();
    let close_count = result.chars().filter(|c| *c == ')').count();
    
    // If we have more close parens than open, strip trailing ones
    if close_count > open_count {
        let excess = close_count - open_count;
        for _ in 0..excess {
            if result.ends_with(')') {
                result.pop();
            } else if result.ends_with("))") {
                result = result.trim_end_matches(')').to_string();
            }
        }
    }
    
    result
}

pub fn normalize_option(opt: &Option<String>) -> Option<String> {
    opt.as_ref().map(|s| normalize_policy_expression(s))
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

/// Helper function to clean up leftover parentheses from pg_get_viewdef.
/// This handles cases where the SELECT body has extra nested parens around JOINs, ON clauses, etc.
fn cleanup_view_parens(normalized: &str) -> String {
    let mut normalized = normalized.to_string();
    
    // Iteratively collapse nested parentheses (( -> ( and )) -> )
    // pg_get_viewdef wraps JOINs and other constructs in extra parens
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
    
    // Run normalize_sql one more time to clean up any double spaces
    normalize_sql(&normalized)
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

    // Try AST-based normalization first (handles redundant parens, casts, etc. robustly)
    if let Some(ast_normalized) = normalize_via_ast(&s) {
        // Apply post-AST string cleanup for leftover parentheses from pg_get_viewdef
        return cleanup_view_parens(&normalize_sql(&ast_normalized));
    }
    
    // Fallback to string-based normalization logic if parsing fails
    // Apply standard SQL normalization (collapses whitespace, lowercases, normalizes parens)
    let mut normalized = normalize_sql(&s);
    
    // Normalize interval syntax: interval '7 days' -> '7 days'
    // Postgres normalization usually converts `interval 'x'` to `'x'::interval`
    if normalized.contains("interval '") {
         normalized = normalized.replace("interval '", "'");
    }

    // Strip type casts commonly found in introspected views (e.g. (0)::bigint -> 0)
    // IMPORTANT: Longer matches must come first! e.g. ::interval before ::int
    let type_cast_suffixes = [
        "::text[]", "::text", 
        "::integer[]", "::integer", "::interval", "::int[]", "::int",
        "::bigint[]", "::bigint", "::smallint[]", "::smallint",
        "::character varying[]", "::character varying", "::varchar[]", "::varchar",
        "::boolean", "::bool", "::numeric", "::jsonb[]", "::jsonb", "::uuid[]", "::uuid",
        "::float", "::double precision", "::regclass", "::regtype",
        "::date", "::time", "::timestamp", "::timestamptz",
    ];

    for suffix in type_cast_suffixes {
        normalized = normalized.replace(suffix, "");
    }
    
    // Strip trailing semicolon - pg_get_viewdef includes it, sqlparser doesn't
    normalized = normalized.trim_end_matches(';').to_string();
    
    // Final pass to clean up any redundant parens that might have been left by removing casts
    let tokens_to_unwrap = ["0", "0.0", "1", "null", "true", "false"];
    for token in tokens_to_unwrap {
        let wrapped = format!("({})", token);
        normalized = normalized.replace(&format!(",{}", wrapped), &format!(",{}", token));
        normalized = normalized.replace(&format!("={}", wrapped), &format!("={}", token));
        normalized = normalized.replace(&format!(" {}", wrapped), &format!(" {}", token));
        normalized = normalized.replace(&format!("({}", wrapped), &format!("({}", token));
        normalized = normalized.replace(&format!("[{}", wrapped), &format!("[{}", token));
        normalized = normalized.replace(&format!(">{}", wrapped), &format!(">{}", token));
        normalized = normalized.replace(&format!("<{}", wrapped), &format!("<{}", token));
    }
    
    // Apply the parenthesis cleanup (handles nested parens, FROM/ON/GROUP BY, etc.)
    cleanup_view_parens(&normalized)
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



