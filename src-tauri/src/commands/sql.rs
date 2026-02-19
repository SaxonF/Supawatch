use std::sync::Arc;
use tauri::{AppHandle, Manager};
use uuid::Uuid;
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use crate::state::AppState;

/// Validate SQL syntax using sqlparser
#[tauri::command]
pub fn validate_sql(sql: String) -> Result<(), String> {
    let dialect = PostgreSqlDialect {};
    Parser::parse_sql(&dialect, &sql)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Build a concise schema description for AI context
fn build_schema_context(schema: &crate::schema::DbSchema) -> String {
    let mut context = String::new();
    
    // Add tables with columns
    if !schema.tables.is_empty() {
        context.push_str("Tables:\n");
        for (key, table) in &schema.tables {
            // Extract table name from key (e.g., "\"public\".\"users\"" -> "users")
            let table_name = key.split('.').last().unwrap_or(key).replace('"', "");
            context.push_str(&format!("- {} (", table_name));
            
            let cols: Vec<String> = table.columns.iter()
                .map(|(name, col)| {
                    let mut desc = format!("{} {}", name, col.data_type);
                    if col.is_primary_key {
                        desc.push_str(" PK");
                    }
                    if !col.is_nullable {
                        desc.push_str(" NOT NULL");
                    }
                    desc
                })
                .collect();
            context.push_str(&cols.join(", "));
            context.push_str(")\n");
            
            // Add foreign keys for relationships
            for fk in &table.foreign_keys {
                let columns = fk.columns.join(", ");
                let foreign_columns = fk.foreign_columns.join(", ");
                context.push_str(&format!("  FK: ({}) -> {}.{}.({})\n", 
                    columns, fk.foreign_schema, fk.foreign_table, foreign_columns));
            }
        }
    }
    
    // Add enums
    if !schema.enums.is_empty() {
        context.push_str("\nEnums:\n");
        for (key, enum_info) in &schema.enums {
            let enum_name = key.split('.').last().unwrap_or(key).replace('"', "");
            context.push_str(&format!("- {} = [{}]\n", enum_name, enum_info.values.join(", ")));
        }
    }
    
    context
}

/// Convert natural language or invalid SQL to valid SQL using OpenAI
#[tauri::command]
pub async fn convert_with_ai(
    app_handle: AppHandle,
    project_id: String,
    input: String,
    error_message: Option<String>,
) -> Result<String, String> {
    let state = app_handle.state::<Arc<AppState>>();
    
    let api_key = state
        .get_openai_key()
        .await
        .ok_or("OpenAI API key not configured. Please add it in Settings.")?;

    // Get schema context - check cache first, then introspect if needed
    let schema_context = {
        let uuid = Uuid::parse_str(&project_id).map_err(|e| e.to_string())?;
        let project = state.get_project(uuid).await.map_err(|e| e.to_string())?;
        
        if project.supabase_project_ref.is_none() {
            String::new()
        } else {
            // Check cache first
            if let Some(cached_schema) = state.get_cached_schema(uuid).await {
                build_schema_context(&cached_schema)
            } else {
                // Cache miss - fetch and cache
                let project_ref = project.supabase_project_ref.as_ref().unwrap();
                let api = state.get_api_client().await.map_err(|e| e.to_string())?;
                let introspector = crate::introspection::Introspector::new(&api, project_ref.clone());
                
                match introspector.introspect().await {
                    Ok(schema) => {
                        let context = build_schema_context(&schema);
                        // Cache the schema for future use
                        state.set_cached_schema(uuid, schema).await;
                        context
                    }
                    Err(e) => {
                        eprintln!("Failed to introspect schema for AI context: {}", e);
                        String::new()
                    }
                }
            }
        }
    };

    let system_prompt = format!(
        r#"You are a PostgreSQL SQL expert. Convert the user's input into valid PostgreSQL SQL.

Rules:
1. Preserve the original intent of the query. If the user is calling a function (e.g. cron.schedule), DO NOT change it to a SELECT statement.
2. If the input is invalid SQL, fix the syntax errors while maintaining the logic.
3. If the input is natural language, or partially natural language, convert it to a valid SQL query.
4. Return ONLY the SQL query, no explanations or markdown.
5. Use standard PostgreSQL syntax.
6. For data retrieval SELECT queries (NOT function calls), include reasonable LIMIT if not specified.
7. Use the exact table and column names from the schema below.

Database Schema:
{}

Return only valid PostgreSQL SQL."#,
        if schema_context.is_empty() { "No schema available".to_string() } else { schema_context }
    );

    let user_content = if let Some(err) = error_message {
        format!("Query: {}\n\nError: {}\n\nPlease fix the query to resolve the error.", input, err)
    } else {
        input
    };

    println!("--- AI Request Debug Start ---");
    println!("System Prompt:\n{}", system_prompt);
    println!("\nUser Content:\n{}", user_content);
    println!("--- AI Request Debug End ---");

    let response = state.http_client
        .post("https://api.openai.com/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [
                {
                    "role": "system",
                    "content": system_prompt
                },
                {
                    "role": "user",
                    "content": user_content
                }
            ],
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to call OpenAI API: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("OpenAI API error ({}): {}", status, error_text));
    }

    let json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenAI response: {}", e))?;

    let sql = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("Invalid response from OpenAI")?
        .trim()
        .to_string();

    // Clean up any markdown code blocks if present
    let sql = sql
        .trim_start_matches("```sql")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    Ok(sql)
}
