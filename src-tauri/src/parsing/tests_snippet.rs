#[cfg(test)]
mod tests {
    use crate::generator::objects::generate_create_function;
    use crate::parsing::parse_schema_sql;

    #[test]
    fn test_issue_security_definer_user_snippet() {
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
        let files = vec![("test.sql".to_string(), sql.to_string())];
        let schema = parse_schema_sql(&files).expect("Failed to parse SQL");
        println!("{:#?}", schema.functions.keys());
        
        let func = schema.functions.get("\"public\".\"sync_agent_task_cron\"()").unwrap();
        assert!(func.security_definer, "Security definer should be true!");
        
        let gen_sql = generate_create_function(func);
        println!("Generated SQL:\n{}", gen_sql);
        assert!(gen_sql.to_uppercase().contains("SECURITY DEFINER"), "Generated SQL must contain SECURITY DEFINER");
    }
}
