use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;
use keyring::Entry;

use crate::models::{AppData, LogEntry, Project};
use crate::schema::DbSchema;
use crate::supabase_api::SupabaseApi;

const SERVICE_NAME: &str = "harbor";
const ACCESS_TOKEN_KEY: &str = "access_token";
const OPENAI_KEY_KEY: &str = "openai_key";

#[derive(Error, Debug)]
pub enum StateError {
    #[error("Project not found: {0}")]
    ProjectNotFound(Uuid),
    #[error("Failed to read data file: {0}")]
    ReadError(String),
    #[error("Failed to write data file: {0}")]
    WriteError(String),
    #[error("Failed to parse data: {0}")]
    ParseError(String),
    #[error("Access token not configured")]
    NoAccessToken,
}

pub type WatcherHandle = notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>;

pub struct AppState {
    pub data: RwLock<AppData>,
    pub logs: RwLock<Vec<LogEntry>>,
    pub watchers: RwLock<HashMap<Uuid, WatcherHandle>>,
    pub openai_key: RwLock<Option<String>>,
    pub schema_cache: RwLock<HashMap<Uuid, DbSchema>>,
    pub http_client: reqwest::Client,
    data_path: PathBuf,
}

impl AppState {
    pub fn new() -> Self {
        const HTTP_TIMEOUT_SECS: u64 = 120;
        const HTTP_CONNECT_TIMEOUT_SECS: u64 = 15;

        let data_dir = Self::get_data_dir();
        let data_path = data_dir.join("data.json");
        
        let (mut data, _) = Self::load_data(&data_path).unwrap_or_default();

        println!("[TOKEN] Loading token from keychain...");
        let access_token = match Entry::new(SERVICE_NAME, ACCESS_TOKEN_KEY) {
            Ok(entry) => match entry.get_password() {
                Ok(pwd) => {
                    println!("[TOKEN] Successfully loaded token from keychain");
                    Some(pwd)
                },
                Err(_) => {
                    println!("[TOKEN] No token found in keychain");
                    None
                }
            },
            Err(e) => {
                eprintln!("[TOKEN] Failed to access keychain: {}", e);
                None
            }
        };
        data.access_token = access_token.clone();

        println!("[OPENAI] Loading OpenAI key from keychain...");
        let openai_key_val = match Entry::new(SERVICE_NAME, OPENAI_KEY_KEY) {
            Ok(entry) => match entry.get_password() {
                Ok(pwd) => {
                    println!("[OPENAI] Successfully loaded OpenAI key from keychain");
                    Some(pwd)
                },
                Err(_) => None
            },
            Err(_) => None
        };

        // Initialize state with loaded values
        let openai_key = RwLock::new(openai_key_val);

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .connect_timeout(Duration::from_secs(HTTP_CONNECT_TIMEOUT_SECS))
            .build()
            .unwrap_or_else(|e| {
                eprintln!("[HTTP] Failed to build client with timeouts: {}", e);
                reqwest::Client::new()
            });

        Self {
            data: RwLock::new(data),
            logs: RwLock::new(Vec::new()),
            watchers: RwLock::new(HashMap::new()),
            openai_key,
            schema_cache: RwLock::new(HashMap::new()),
            http_client,
            data_path,
        }
    }
    
    fn get_data_dir() -> PathBuf {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("harbor");

        fs::create_dir_all(&data_dir).ok();
        data_dir
    }

    fn load_data(path: &PathBuf) -> Result<(AppData, Option<String>), StateError> {
        if !path.exists() {
            return Ok((AppData::default(), None));
        }

        let content = fs::read_to_string(path)
            .map_err(|e| StateError::ReadError(e.to_string()))?;

        // Parse as Value first to check for legacy token
        let v: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| StateError::ParseError(e.to_string()))?;

        let legacy_token = v.get("access_token")
            .and_then(|t| t.as_str())
            .map(|t| t.to_string());

        let data: AppData = serde_json::from_value(v)
            .map_err(|e| StateError::ParseError(e.to_string()))?;

        Ok((data, legacy_token))
    }
    

    pub async fn save(&self) -> Result<(), StateError> {
        let data = self.data.read().await;
        // The access_token field is skipped, so it won't be written to disk.
        let content = serde_json::to_string_pretty(&*data)
            .map_err(|e| StateError::ParseError(e.to_string()))?;

        fs::write(&self.data_path, content)
            .map_err(|e| StateError::WriteError(e.to_string()))
    }

    // Project operations
    pub async fn add_project(&self, project: Project) -> Result<Project, StateError> {
        let mut data = self.data.write().await;
        let project_clone = project.clone();
        data.projects.push(project);
        drop(data);
        self.save().await?;
        Ok(project_clone)
    }

    pub async fn get_projects(&self) -> Vec<Project> {
        let data = self.data.read().await;
        data.projects.clone()
    }

    pub async fn get_project(&self, id: Uuid) -> Result<Project, StateError> {
        let data = self.data.read().await;
        data.projects
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or(StateError::ProjectNotFound(id))
    }

    pub async fn update_project(&self, project: Project) -> Result<Project, StateError> {
        let mut data = self.data.write().await;
        let idx = data
            .projects
            .iter()
            .position(|p| p.id == project.id)
            .ok_or(StateError::ProjectNotFound(project.id))?;

        data.projects[idx] = project.clone();
        drop(data);
        self.save().await?;
        Ok(project)
    }

    pub async fn delete_project(&self, id: Uuid) -> Result<(), StateError> {
        // Stop watcher if running
        self.stop_watcher(id).await;

        let mut data = self.data.write().await;
        let idx = data
            .projects
            .iter()
            .position(|p| p.id == id)
            .ok_or(StateError::ProjectNotFound(id))?;

        data.projects.remove(idx);
        drop(data);
        self.save().await
    }

    pub async fn set_project_watching(&self, id: Uuid, watching: bool) -> Result<(), StateError> {
        let mut data = self.data.write().await;
        let project = data
            .projects
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or(StateError::ProjectNotFound(id))?;

        project.is_watching = watching;
        project.updated_at = chrono::Utc::now();
        drop(data);
        self.save().await
    }

    // Watcher operations
    pub async fn add_watcher(&self, project_id: Uuid, watcher: WatcherHandle) {
        let mut watchers = self.watchers.write().await;
        watchers.insert(project_id, watcher);
    }

    pub async fn stop_watcher(&self, project_id: Uuid) {
        let mut watchers = self.watchers.write().await;
        watchers.remove(&project_id);
    }

    pub async fn is_watching(&self, project_id: Uuid) -> bool {
        let watchers = self.watchers.read().await;
        watchers.contains_key(&project_id)
    }

    // Log operations
    pub async fn add_log(&self, log: LogEntry) {
        let mut logs = self.logs.write().await;
        logs.push(log);

        // Keep only last 1000 logs
        if logs.len() > 1000 {
            let drain_count = logs.len() - 1000;
            logs.drain(0..drain_count);
        }
    }

    pub async fn get_logs(&self, project_id: Option<Uuid>, limit: usize) -> Vec<LogEntry> {
        let logs = self.logs.read().await;
        let filtered: Vec<_> = logs
            .iter()
            .filter(|log| {
                project_id.is_none() || log.project_id == project_id
            })
            .cloned()
            .collect();

        filtered.into_iter().rev().take(limit).collect()
    }

    pub async fn clear_logs(&self, project_id: Option<Uuid>) {
        let mut logs = self.logs.write().await;
        if let Some(pid) = project_id {
            logs.retain(|log| log.project_id != Some(pid));
        } else {
            logs.clear();
        }
    }

    // Access token operations
    pub async fn set_access_token(&self, token: String) -> Result<(), StateError> {
        println!("[TOKEN] set_access_token called");
        
        let entry = Entry::new(SERVICE_NAME, ACCESS_TOKEN_KEY).map_err(|e| StateError::WriteError(e.to_string()))?;
        entry.set_password(&token).map_err(|e| StateError::WriteError(e.to_string()))?;

        let mut data = self.data.write().await;
        data.access_token = Some(token);
        drop(data);
        Ok(())
    }

    pub async fn get_access_token(&self) -> Option<String> {
        let data = self.data.read().await;
        data.access_token.clone()
    }

    pub async fn clear_access_token(&self) -> Result<(), StateError> {
        let entry = Entry::new(SERVICE_NAME, ACCESS_TOKEN_KEY).map_err(|e| StateError::WriteError(e.to_string()))?;
        entry.delete_credential().map_err(|e| StateError::WriteError(e.to_string()))?;

        let mut data = self.data.write().await;
        data.access_token = None;
        drop(data);
        self.save().await
    }

    pub async fn has_access_token(&self) -> bool {
        let data = self.data.read().await;
        data.access_token.is_some()
    }

    /// Get a Supabase API client using the stored access token
    pub async fn get_api_client(&self) -> Result<SupabaseApi, StateError> {
        let token = self.get_access_token().await.ok_or(StateError::NoAccessToken)?;
        Ok(SupabaseApi::new(token, self.http_client.clone()))
    }

    // OpenAI key operations
    pub async fn set_openai_key(&self, key: String) -> Result<(), StateError> {
        println!("[OPENAI] set_openai_key called");
        let entry = Entry::new(SERVICE_NAME, OPENAI_KEY_KEY).map_err(|e| StateError::WriteError(e.to_string()))?;
        entry.set_password(&key).map_err(|e| StateError::WriteError(e.to_string()))?;
        
        let mut openai_key = self.openai_key.write().await;
        *openai_key = Some(key);
        Ok(())
    }

    pub async fn get_openai_key(&self) -> Option<String> {
        let openai_key = self.openai_key.read().await;
        openai_key.clone()
    }

    pub async fn clear_openai_key(&self) -> Result<(), StateError> {
        let entry = Entry::new(SERVICE_NAME, OPENAI_KEY_KEY).map_err(|e| StateError::WriteError(e.to_string()))?;
        entry.delete_credential().map_err(|e| StateError::WriteError(e.to_string()))?;
        
        let mut openai_key = self.openai_key.write().await;
        *openai_key = None;
        Ok(())
    }

    pub async fn has_openai_key(&self) -> bool {
        let openai_key = self.openai_key.read().await;
        openai_key.is_some()
    }

    // Schema cache operations
    pub async fn get_cached_schema(&self, project_id: Uuid) -> Option<DbSchema> {
        let cache = self.schema_cache.read().await;
        cache.get(&project_id).cloned()
    }

    pub async fn set_cached_schema(&self, project_id: Uuid, schema: DbSchema) {
        let mut cache = self.schema_cache.write().await;
        cache.insert(project_id, schema);
    }

    pub async fn clear_cached_schema(&self, project_id: Uuid) {
        let mut cache = self.schema_cache.write().await;
        cache.remove(&project_id);
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
