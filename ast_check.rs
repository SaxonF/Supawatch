use sqlparser::parser::Parser;
use sqlparser::dialect::PostgreSqlDialect;

fn main() {
    let dialect = PostgreSqlDialect {};
    let sql1 = "grant usage on schema cron to postgres;";
    let sql2 = "grant all privileges on all tables in schema cron to postgres;";
    
    let ast1 = Parser::parse_sql(&dialect, sql1).unwrap();
    println!("AST 1: {:#?}", ast1[0]);

    let ast2 = Parser::parse_sql(&dialect, sql2).unwrap();
    println!("AST 2: {:#?}", ast2[0]);
}
