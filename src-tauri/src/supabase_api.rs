use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SUPABASE_API_BASE: &str = "https://api.supabase.com";

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },
    #[error("Missing access token")]
    MissingToken,
    #[error("Missing project reference")]
    MissingProjectRef,
    #[error("File read error: {0}")]
    FileReadError(String),
}

#[derive(Debug, Serialize)]
struct QueryRequest {
    query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    read_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct QueryResponse {
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub organization_id: String,
    pub region: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
struct CreateProjectBody {
    name: String,
    organization_id: String,
    db_pass: String,
    region: String,
    plan: String,
}

#[derive(Debug, Deserialize)]
pub struct EdgeFunction {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub status: String,
    pub version: i32,
    pub created_at: serde_json::Value,
    pub updated_at: serde_json::Value,
    #[serde(default)]
    pub entrypoint_path: Option<String>,
}

#[derive(Debug, Serialize)]
struct FunctionMetadata {
    entrypoint_path: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    import_map_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verify_jwt: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct DeployResponse {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub version: i32,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiKey {
    pub api_key: String,
    pub id: String,
    pub name: String,
    pub role: Option<String>,
    #[serde(rename = "type")]
    pub key_type: String,
    #[serde(default)]
    pub reveal: bool,
}

#[derive(Debug, Serialize)]
struct CreateApiKeyBody {
    #[serde(rename = "type")]
    key_type: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret_jwt_template: Option<serde_json::Value>,
}

/// Represents a single file in a function
pub struct FunctionFile {
    pub name: String,
    pub content: Vec<u8>,
}

/// Metadata from function body response
#[derive(Debug, Deserialize, Default)]
pub struct FunctionBodyMetadata {
    #[serde(default)]
    pub deno2_entrypoint_path: Option<String>,
}

pub struct FunctionBody {
    pub content_type: String,
    pub data: Vec<u8>,
    /// Individual files (populated when multipart response)
    pub files: Vec<FunctionFile>,
    /// Metadata (populated when multipart response)
    pub metadata: FunctionBodyMetadata,
}

pub struct SupabaseApi {
    client: reqwest::Client,
    access_token: String,
}

impl SupabaseApi {
    pub fn new(access_token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            access_token,
        }
    }

    fn auth_header(&self) -> String {
        format!("Bearer {}", self.access_token)
    }

    /// List all projects accessible by the access token
    pub async fn list_projects(&self) -> Result<Vec<Project>, ApiError> {
        let url = format!("{}/v1/projects", SUPABASE_API_BASE);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        Ok(response.json().await?)
    }

    /// List all organizations
    pub async fn list_organizations(&self) -> Result<Vec<Organization>, ApiError> {
        let url = format!("{}/v1/organizations", SUPABASE_API_BASE);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        Ok(response.json().await?)
    }

    /// Create a new project
    pub async fn create_project(
        &self,
        name: &str,
        organization_id: &str,
        db_pass: &str,
        region: &str,
    ) -> Result<Project, ApiError> {
        let url = format!("{}/v1/projects", SUPABASE_API_BASE);

        let body = CreateProjectBody {
            name: name.to_string(),
            organization_id: organization_id.to_string(),
            db_pass: db_pass.to_string(),
            region: region.to_string(),
            plan: "free".to_string(), // Defaulting to free plan
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        Ok(response.json().await?)
    }

    /// Get a specific project by reference
    pub async fn get_project(&self, project_ref: &str) -> Result<Project, ApiError> {
        let url = format!("{}/v1/projects/{}", SUPABASE_API_BASE, project_ref);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        Ok(response.json().await?)
    }

    /// Run a SQL query against the project's database
    pub async fn run_query(
        &self,
        project_ref: &str,
        query: &str,
        read_only: bool,
    ) -> Result<QueryResponse, ApiError> {
        let url = format!(
            "{}/v1/projects/{}/database/query",
            SUPABASE_API_BASE, project_ref
        );

        let body = QueryRequest {
            query: query.to_string(),
            read_only: Some(read_only),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        let body_text = response.text().await?;
        
        let val: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| {
                 let snippet: String = body_text.chars().take(200).collect();
                 ApiError::ApiError { 
                     status: 200, 
                     message: format!("Failed to parse JSON: {}. Body: {}", e, snippet) 
                 }
            })?;

        if val.is_array() {
            return Ok(QueryResponse {
                result: Some(val),
                error: None,
            });
        }

        if let serde_json::Value::Object(ref map) = val {
            if map.contains_key("result") || map.contains_key("error") {
                 // Try standard deserialization
                 if let Ok(resp) = serde_json::from_value::<QueryResponse>(val.clone()) {
                      // Validate inner result is array if present
                      if let Some(res) = &resp.result {
                          if !res.is_array() {
                               return Err(ApiError::ApiError { 
                                     status: 200, 
                                     message: format!("Query 'result' is not an array: {:?}. Body: {}", res, body_text)
                               });
                          }
                      }
                      return Ok(resp);
                 }
            }
        }
        
        // If we reached here, it's an object but not a query response, or failed to deserialize
        let snippet: String = body_text.chars().take(200).collect();
        Err(ApiError::ApiError { 
             status: 200, 
             message: format!("Unexpected response format. Not an array and not a result/error object. Body: {}", snippet) 
        })
    }

    /// List all edge functions for a project
    pub async fn list_functions(&self, project_ref: &str) -> Result<Vec<EdgeFunction>, ApiError> {
        let url = format!("{}/v1/projects/{}/functions", SUPABASE_API_BASE, project_ref);

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        let body_text = response.text().await?;
        match serde_json::from_str::<Vec<EdgeFunction>>(&body_text) {
            Ok(funcs) => Ok(funcs),
            Err(e) => {
                let snippet: String = body_text.chars().take(200).collect();
                Err(ApiError::ApiError { 
                    status: 200, 
                    message: format!("Failed to parse functions list: {}. Body: {}", e, snippet) 
                })
            }
        }
    }

    /// Deploy an edge function
    ///
    /// files is a vector of (relative_path, content) pairs for all files in the function
    /// entrypoint is the main file name (e.g., "index.ts")
    pub async fn deploy_function(
        &self,
        project_ref: &str,
        slug: &str,
        name: &str,
        entrypoint: &str,
        files: Vec<(String, Vec<u8>)>,
    ) -> Result<DeployResponse, ApiError> {
        let url = format!(
            "{}/v1/projects/{}/functions/deploy?slug={}",
            SUPABASE_API_BASE, project_ref, slug
        );

        // Detect import map file (deno.json or import_map.json)
        let import_map_path = files.iter()
            .find(|(path, _)| {
                let lower = path.to_lowercase();
                lower == "deno.json" || lower == "deno.jsonc" || 
                lower == "import_map.json" || lower.ends_with("/deno.json") ||
                lower.ends_with("/deno.jsonc") || lower.ends_with("/import_map.json")
            })
            .map(|(path, _)| path.clone());

        let metadata = FunctionMetadata {
            entrypoint_path: entrypoint.to_string(),
            name: name.to_string(),
            import_map_path,
            verify_jwt: Some(false),
        };

        let metadata_json = serde_json::to_string(&metadata)
            .map_err(|e| ApiError::FileReadError(e.to_string()))?;

        let mut form = Form::new()
            .text("metadata", metadata_json);

        // Add all files to the form
        for (path, content) in files {
            let mime_type = if path.ends_with(".ts") {
                "application/typescript"
            } else if path.ends_with(".js") {
                "application/javascript"
            } else if path.ends_with(".json") {
                "application/json"
            } else {
                "application/octet-stream"
            };
            
            form = form.part(
                "file",
                Part::bytes(content)
                    .file_name(path)
                    .mime_str(mime_type)
                    .map_err(|e| ApiError::FileReadError(e.to_string()))?,
            );
        }

        let response = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        Ok(response.json().await?)
    }

    /// Delete an edge function
    pub async fn delete_function(
        &self,
        project_ref: &str,
        function_slug: &str,
    ) -> Result<(), ApiError> {
        let url = format!(
            "{}/v1/projects/{}/functions/{}",
            SUPABASE_API_BASE, project_ref, function_slug
        );

        let response = self
            .client
            .delete(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        Ok(())
    }

    /// Get edge function body (code)
    /// 
    /// Requests multipart/form-data format to get individual source files and metadata
    pub async fn get_function_body(
        &self,
        project_ref: &str,
        function_slug: &str,
    ) -> Result<FunctionBody, ApiError> {
        let url = format!(
            "{}/v1/projects/{}/functions/{}/body",
            SUPABASE_API_BASE, project_ref, function_slug
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "multipart/form-data")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();

        let data = response.bytes().await?.to_vec();
        
        // Parse multipart if available
        let mut files = Vec::new();
        let mut metadata = FunctionBodyMetadata::default();
        
        if content_type.contains("multipart/form-data") {
            // Extract boundary from content-type
            if let Some(boundary) = content_type
                .split(';')
                .filter_map(|part| {
                    let part = part.trim();
                    if part.starts_with("boundary=") {
                        Some(part.trim_start_matches("boundary=").trim_matches('"'))
                    } else {
                        None
                    }
                })
                .next()
            {
                // Parse multipart data
                let boundary_bytes = format!("--{}", boundary);
                let _parts: Vec<&[u8]> = data
                    .split(|&b| b == b'\n')
                    .collect::<Vec<_>>()
                    .split(|line| {
                        let line_str = String::from_utf8_lossy(line);
                        line_str.trim() == boundary_bytes || line_str.trim() == format!("{}--", boundary_bytes)
                    })
                    .filter(|p| !p.is_empty())
                    .map(|chunk| {
                        // Rejoin lines in each chunk
                        chunk.join(&b'\n')
                    })
                    .collect::<Vec<Vec<u8>>>()
                    .iter()
                    .map(|v| v.as_slice())
                    .collect();
                
                // Simple multipart parser - look for Content-Disposition headers
                let data_str = String::from_utf8_lossy(&data);
                let boundary_str = format!("--{}", boundary);
                
                for part in data_str.split(&boundary_str) {
                    let part = part.trim();
                    if part.is_empty() || part == "--" {
                        continue;
                    }
                    
                    // Split headers from body (double CRLF or double LF)
                    let body_start = if let Some(pos) = part.find("\r\n\r\n") {
                        pos + 4
                    } else if let Some(pos) = part.find("\n\n") {
                        pos + 2
                    } else {
                        continue;
                    };
                    
                    let headers = &part[..body_start];
                    let body = &part[body_start..];
                    
                    // Check for filename in Content-Disposition
                    if let Some(filename) = headers
                        .lines()
                        .find(|line| line.to_lowercase().contains("content-disposition"))
                        .and_then(|line| {
                            line.split(';')
                                .find(|part| part.trim().starts_with("filename="))
                                .map(|part| {
                                    part.trim()
                                        .trim_start_matches("filename=")
                                        .trim_matches('"')
                                        .to_string()
                                })
                        })
                    {
                        // This is a file
                        files.push(FunctionFile {
                            name: filename,
                            content: body.trim().as_bytes().to_vec(),
                        });
                    } else {
                        // This might be metadata (JSON without filename)
                        if let Ok(parsed_meta) = serde_json::from_str::<FunctionBodyMetadata>(body.trim()) {
                            metadata = parsed_meta;
                        }
                    }
                }
            }
        }

        Ok(FunctionBody { 
            content_type, 
            data,
            files,
            metadata,
        })
    }

    /// Get the current database schema (useful for diffing)
    pub async fn get_schema(&self, project_ref: &str) -> Result<String, ApiError> {
        // Query to get the current schema
        let query = r#"
            SELECT
                table_schema,
                table_name,
                column_name,
                data_type,
                is_nullable,
                column_default
            FROM information_schema.columns
            WHERE table_schema NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
            ORDER BY table_schema, table_name, ordinal_position;
        "#;

        let result = self.run_query(project_ref, query, true).await?;

        Ok(serde_json::to_string_pretty(&result.result).unwrap_or_default())
    }

    /// Query project logs using SQL
    ///
    /// Available log sources: edge_logs, postgres_logs, auth_logs, realtime_logs, storage_logs, postgrest_logs
    /// If no SQL provided, defaults to querying edge_logs
    /// Timestamp range must be no more than 24 hours
    pub async fn query_logs(
        &self,
        project_ref: &str,
        sql: Option<&str>,
        iso_timestamp_start: Option<&str>,
        iso_timestamp_end: Option<&str>,
    ) -> Result<serde_json::Value, ApiError> {
        let mut url = format!(
            "{}/v1/projects/{}/analytics/endpoints/logs.all",
            SUPABASE_API_BASE, project_ref
        );

        let mut query_params = Vec::new();
        if let Some(sql) = sql {
            query_params.push(format!("sql={}", urlencoding::encode(sql)));
        }
        if let Some(start) = iso_timestamp_start {
            query_params.push(format!("iso_timestamp_start={}", urlencoding::encode(start)));
        }
        if let Some(end) = iso_timestamp_end {
            query_params.push(format!("iso_timestamp_end={}", urlencoding::encode(end)));
        }

        if !query_params.is_empty() {
            url = format!("{}?{}", url, query_params.join("&"));
        }

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        let val: serde_json::Value = response.json().await?;

        if val.is_array() {
            return Ok(val);
        }

        if let Some(obj) = val.as_object() {
            if let Some(error) = obj.get("error") {
                if !error.is_null() {
                    let msg = format!("Supabase API Error: {}", error);
                    return Err(ApiError::ApiError { 
                        status: 200, 
                        message: msg
                    });
                }
            }
            if let Some(res) = obj.get("result") {
                return Ok(res.clone());
            }
        }

        Ok(val)
    }

    /// Get edge function logs for the last N minutes
    pub async fn get_edge_function_logs(
        &self,
        project_ref: &str,
        function_name: Option<&str>,
        minutes: u32,
    ) -> Result<serde_json::Value, ApiError> {
        let now = chrono::Utc::now();
        let start = now - chrono::Duration::minutes(minutes as i64);

        let sql = if let Some(name) = function_name {
            format!(
                r#"select 
                    id, 
                    datetime(t.timestamp) as timestamp, 
                    event_message, 
                    m.function_id, 
                    m.execution_time_ms, 
                    m.deployment_id, 
                    m.version, 
                    r.method, 
                    r.url, 
                    resp.status_code 
                   from function_edge_logs as t
                   cross join unnest(metadata) as m
                   cross join unnest(m.request) as r
                   cross join unnest(m.response) as resp
                   where m.function_id = '{}'
                   order by timestamp desc
                   limit 100"#,
                name
            )
        } else {
            r#"select 
                id, 
                datetime(t.timestamp) as timestamp, 
                event_message, 
                m.function_id, 
                m.execution_time_ms, 
                m.deployment_id, 
                m.version, 
                r.method, 
                r.url, 
                resp.status_code 
               from function_edge_logs as t
               left join unnest(metadata) as m
               left join unnest(m.request) as r
               left join unnest(m.response) as resp
               order by timestamp desc
               limit 100"#
                .to_string()
        };

        self.query_logs(
            project_ref,
            Some(&sql),
            Some(&start.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
            Some(&now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        )
        .await
    }

    /// Get postgres logs for the last N minutes
    pub async fn get_postgres_logs(
        &self,
        project_ref: &str,
        minutes: u32,
    ) -> Result<serde_json::Value, ApiError> {
        let now = chrono::Utc::now();
        let start = now - chrono::Duration::minutes(minutes as i64);

        // Select metadata to get error_severity, user_name, query etc.
        // Filter to only show errors to reduce noise
        let sql = r#"select 
                    identifier, 
                    postgres_logs.timestamp, 
                    id, 
                    event_message, 
                    parsed.error_severity, 
                    parsed.detail, 
                    parsed.hint 
                    from postgres_logs
                    cross join unnest(metadata) as m
                    cross join unnest(m.parsed) as parsed
                    order by timestamp desc
                    limit 100"#;

        self.query_logs(
            project_ref,
            Some(sql),
            Some(&start.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
            Some(&now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        )
        .await
    }

    /// Get auth logs for the last N minutes
    pub async fn get_auth_logs(
        &self,
        project_ref: &str,
        minutes: u32,
    ) -> Result<serde_json::Value, ApiError> {
        let now = chrono::Utc::now();
        let start = now - chrono::Duration::minutes(minutes as i64);

        // Select metadata to get detail fields
        let sql = r#"select id, datetime(timestamp) as timestamp, event_message, metadata
                     from auth_logs
                     order by timestamp desc
                     limit 100"#;

        self.query_logs(
            project_ref,
            Some(sql),
            Some(&start.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
            Some(&now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        )
        .await
    }

    /// Get API keys for a project
    pub async fn get_api_keys(&self, project_ref: &str) -> Result<Vec<ApiKey>, ApiError> {
        let url = format!(
            "{}/v1/projects/{}/api-keys?reveal=true",
            SUPABASE_API_BASE, project_ref
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", self.auth_header())
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        Ok(response.json().await?)
    }

    /// Create a new API key
    pub async fn create_api_key(
        &self,
        project_ref: &str,
        key_type: &str,
        name: &str,
        role: Option<&str>,
    ) -> Result<ApiKey, ApiError> {
        let url = format!(
            "{}/v1/projects/{}/api-keys?reveal=true",
            SUPABASE_API_BASE, project_ref
        );

        let body = CreateApiKeyBody {
            key_type: key_type.to_string(),
            name: name.to_string(),
            secret_jwt_template: role.map(|r| {
                serde_json::json!({
                    "role": r
                })
            }),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            return Err(ApiError::ApiError { status, message });
        }

        Ok(response.json().await?)
    }

    /// Ensure required API keys exist (publishable and secret)
    /// Returns the publishable key
    pub async fn ensure_api_keys(&self, project_ref: &str) -> Result<String, ApiError> {
        let keys = self.get_api_keys(project_ref).await?;

        // Check for publishable key
        let publishable_key = keys.iter()
            .find(|k| k.key_type == "publishable")
            .map(|k| k.api_key.clone());

        let final_publishable_key = if let Some(pk) = publishable_key {
            pk
        } else {
             // Create publishable key
             let k = self.create_api_key(project_ref, "publishable", "default", None).await?;
             k.api_key
        };

        // Check for secret key
        let secret_key_exists = keys.iter().any(|k| k.key_type == "secret");
        if !secret_key_exists {
            // Create secret key (service_role)
            let _ = self.create_api_key(project_ref, "secret", "default", Some("service_role")).await?;
        }

        Ok(final_publishable_key)
    }
}
