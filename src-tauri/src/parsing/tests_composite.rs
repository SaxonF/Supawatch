use crate::parsing::parse_schema_sql;

#[test]
fn test_parse_composite_foreign_key() {
    let sql = r#"
CREATE TABLE "users" (
    "id" uuid NOT NULL,
    "group_id" uuid NOT NULL,
    PRIMARY KEY ("id", "group_id")
);

CREATE TABLE "posts" (
    "id" uuid NOT NULL,
    "user_id" uuid,
    "user_group_id" uuid,
    FOREIGN KEY ("user_id", "user_group_id") REFERENCES "users"("id", "group_id")
);
"#;
    let files = vec![("test.sql".to_string(), sql.to_string())];
    let schema = parse_schema_sql(&files).expect("Failed to parse SQL");
    let posts = schema.tables.get("\"public\".\"posts\"").expect("Table not found");

    assert_eq!(posts.foreign_keys.len(), 1, "Should have 1 foreign key");
    let fk = &posts.foreign_keys[0];
    
    assert_eq!(fk.columns, vec!["user_id", "user_group_id"], "Should capture both local columns");
    assert_eq!(fk.foreign_columns, vec!["id", "group_id"], "Should capture both foreign columns");
}
