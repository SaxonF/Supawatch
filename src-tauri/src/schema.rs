use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoleInfo {
    pub name: String,
    pub superuser: bool,
    pub create_db: bool,
    pub create_role: bool,
    pub inherit: bool,
    pub login: bool,
    pub replication: bool,
    pub bypass_rls: bool,
    pub connection_limit: i32,
    pub valid_until: Option<String>,
    pub password: Option<String>, // Usually encrypted or null/hidden
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DbSchema {
    pub tables: HashMap<String, TableInfo>,
    pub enums: HashMap<String, EnumInfo>,
    pub functions: HashMap<String, FunctionInfo>, // Key is signature "name(arg1, arg2)"
    pub roles: HashMap<String, RoleInfo>,
    // New entities
    pub views: HashMap<String, ViewInfo>,
    pub sequences: HashMap<String, SequenceInfo>,
    pub extensions: HashMap<String, ExtensionInfo>,
    pub composite_types: HashMap<String, CompositeTypeInfo>,
    pub domains: HashMap<String, DomainInfo>,
}

impl Default for DbSchema {
    fn default() -> Self {
        Self {
            tables: HashMap::new(),
            enums: HashMap::new(),
            functions: HashMap::new(),
            roles: HashMap::new(),
            views: HashMap::new(),
            sequences: HashMap::new(),
            extensions: HashMap::new(),
            composite_types: HashMap::new(),
            domains: HashMap::new(),
        }
    }
}

impl DbSchema {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TableInfo {
    pub schema: String, // Added schema field
    pub table_name: String,
    pub columns: HashMap<String, ColumnInfo>,
    pub foreign_keys: Vec<ForeignKeyInfo>,
    pub indexes: Vec<IndexInfo>,
    pub triggers: Vec<TriggerInfo>,
    pub rls_enabled: bool,
    pub policies: Vec<PolicyInfo>,
    pub check_constraints: Vec<CheckConstraintInfo>,
    pub comment: Option<String>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ColumnInfo {
    pub column_name: String,
    pub data_type: String,
    pub is_nullable: bool,
    pub column_default: Option<String>,
    pub udt_name: String,
    pub is_primary_key: bool,
    pub is_unique: bool,
    pub is_identity: bool,
    pub identity_generation: Option<String>, // ALWAYS or BY DEFAULT
    pub is_generated: bool,                  // GENERATED ALWAYS AS ... STORED
    pub generation_expression: Option<String>,
    pub collation: Option<String>,
    pub enum_name: Option<String>,
    pub is_array: bool,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForeignKeyInfo {
    pub constraint_name: String,
    pub columns: Vec<String>,
    pub foreign_schema: String,
    pub foreign_table: String,
    pub foreign_columns: Vec<String>,
    pub on_delete: String,
    pub on_update: String,
}

impl Default for ForeignKeyInfo {
    fn default() -> Self {
        Self {
            constraint_name: String::new(),
            columns: vec![],
            foreign_schema: "public".to_string(),
            foreign_table: String::new(),
            foreign_columns: vec![],
            on_delete: "NO ACTION".to_string(),
            on_update: "NO ACTION".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnumInfo {
    pub schema: String,
    pub name: String,
    pub values: Vec<String>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionInfo {
    pub schema: String,
    pub name: String,
    pub args: Vec<FunctionArg>,
    pub return_type: String,
    pub language: String,
    pub definition: String,
    pub volatility: Option<String>,
    pub is_strict: bool,
    pub security_definer: bool,
    pub config_params: Vec<(String, String)>,
    pub grants: Vec<FunctionGrant>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionArg {
    pub name: String,
    // Use type_ instead of type to avoid keyword conflict, though specific serde rename might be needed if JSON expects "type"
    // But based on usage in generator/tests.rs it is `type_`
    #[serde(rename = "type")]
    pub type_: String,
    pub mode: Option<String>,
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionGrant {
    pub grantee: String,
    pub privilege: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewInfo {
    pub schema: String,
    pub name: String,
    pub definition: String,
    pub is_materialized: bool,
    pub columns: Vec<ViewColumnInfo>,
    pub indexes: Vec<IndexInfo>,
    pub comment: Option<String>,
    pub with_options: Vec<String>,
    pub check_option: Option<String>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewColumnInfo {
    pub name: String,
    pub data_type: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SequenceInfo {
    pub schema: String,
    pub name: String,
    pub data_type: String,
    pub start_value: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub increment: i64,
    pub cycle: bool,
    pub cache_size: i64,
    pub owned_by: Option<String>,
    pub comment: Option<String>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionInfo {
    pub name: String,
    pub version: Option<String>,
    pub schema: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompositeTypeInfo {
    pub schema: String,
    pub name: String,
    pub attributes: Vec<CompositeTypeAttribute>,
    pub comment: Option<String>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompositeTypeAttribute {
    pub name: String,
    pub data_type: String, // definition uses data_type
    pub collation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainInfo {
    pub schema: String,
    pub name: String,
    pub base_type: String,
    pub default_value: Option<String>,
    pub is_not_null: bool,
    pub check_constraints: Vec<DomainCheckConstraint>,
    pub collation: Option<String>,
    pub comment: Option<String>,
    pub extension: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainCheckConstraint {
    pub name: Option<String>,
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct IndexInfo {
    pub index_name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub is_primary: bool,
    pub owning_constraint: Option<String>,
    pub index_method: String,
    pub where_clause: Option<String>,
    pub expressions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TriggerInfo {
    pub name: String,
    pub events: Vec<String>,
    pub timing: String,
    pub orientation: String,
    pub function_name: String,
    pub when_clause: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PolicyInfo {
    pub name: String,
    pub cmd: String,
    pub roles: Vec<String>,
    pub qual: Option<String>,
    pub with_check: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct CheckConstraintInfo {
    pub name: String,
    pub expression: String,
    pub columns: Vec<String>,
}
