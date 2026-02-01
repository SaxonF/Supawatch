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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub collation: Option<String>,
    pub enum_name: Option<String>,
    pub is_array: bool,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ForeignKeyInfo {
    pub constraint_name: String,
    pub column_name: String,
    pub foreign_schema: String,
    pub foreign_table: String,
    pub foreign_column: String,
    pub on_delete: String,
    pub on_update: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IndexInfo {
    pub index_name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub is_primary: bool,
    pub owning_constraint: Option<String>,
    pub index_method: String,         // btree, hash, gist, gin, brin, etc.
    pub where_clause: Option<String>, // Partial index condition
    pub expressions: Vec<String>,     // For expression indexes (e.g., LOWER(email))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EnumInfo {
    pub schema: String, // Added schema field
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PolicyInfo {
    pub name: String,
    pub cmd: String,
    pub roles: Vec<String>,
    pub qual: Option<String>,
    pub with_check: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TriggerInfo {
    pub name: String,
    pub events: Vec<String>,
    pub timing: String,
    pub orientation: String,
    pub function_name: String,
    pub when_clause: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionInfo {
    pub schema: String, // Added schema field
    pub name: String,
    pub args: Vec<FunctionArg>,
    pub return_type: String,
    pub language: String,
    pub definition: String,
    pub volatility: Option<String>, // VOLATILE, STABLE, IMMUTABLE
    pub is_strict: bool,            // STRICT / RETURNS NULL ON NULL INPUT
    pub security_definer: bool,     // SECURITY DEFINER
    pub config_params: Vec<(String, String)>, // SET param = value (e.g., search_path = '')
    pub grants: Vec<FunctionGrant>, // GRANT EXECUTE ON FUNCTION ... TO ...
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionArg {
    pub name: String,
    pub type_: String,
    pub mode: Option<String>, // IN, OUT, INOUT, VARIADIC
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionGrant {
    pub grantee: String,   // Role name: "authenticated", "service_role", etc.
    pub privilege: String, // "EXECUTE" for functions
}

// ========================
// New Entity Types
// ========================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewInfo {
    pub schema: String, // Added schema field
    pub name: String,
    pub definition: String,
    pub is_materialized: bool,
    pub columns: Vec<ViewColumnInfo>,
    pub indexes: Vec<IndexInfo>, // Only for materialized views
    pub comment: Option<String>,
    pub with_options: Vec<String>,    // WITH (security_barrier, etc.)
    pub check_option: Option<String>, // LOCAL, CASCADED
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ViewColumnInfo {
    pub name: String,
    pub data_type: String,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SequenceInfo {
    pub schema: String, // Added schema field
    pub name: String,
    pub data_type: String, // smallint, integer, bigint
    pub start_value: i64,
    pub min_value: i64,
    pub max_value: i64,
    pub increment: i64,
    pub cycle: bool,
    pub cache_size: i64,
    pub owned_by: Option<String>, // table.column if owned by a column
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtensionInfo {
    pub name: String,
    pub version: Option<String>,
    pub schema: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompositeTypeInfo {
    pub schema: String, // Added schema field
    pub name: String,
    pub attributes: Vec<CompositeTypeAttribute>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompositeTypeAttribute {
    pub name: String,
    pub data_type: String,
    pub collation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainInfo {
    pub schema: String, // Added schema field
    pub name: String,
    pub base_type: String,
    pub default_value: Option<String>,
    pub is_not_null: bool,
    pub check_constraints: Vec<DomainCheckConstraint>,
    pub collation: Option<String>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DomainCheckConstraint {
    pub name: Option<String>,
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckConstraintInfo {
    pub name: String,
    pub expression: String,
    pub columns: Vec<String>, // Columns involved in the check
}

impl DbSchema {
    pub fn new() -> Self {
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

impl Default for DbSchema {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for FunctionInfo {
    fn default() -> Self {
        Self {
            schema: "public".to_string(),
            name: String::new(),
            args: vec![],
            return_type: "void".to_string(),
            language: "sql".to_string(),
            definition: String::new(),
            volatility: None,
            is_strict: false,
            security_definer: false,
            config_params: vec![],
            grants: vec![],
        }
    }
}

impl Default for IndexInfo {
    fn default() -> Self {
        Self {
            index_name: String::new(),
            columns: vec![],
            is_unique: false,
            is_primary: false,
            owning_constraint: None,
            index_method: "btree".to_string(),
            where_clause: None,
            expressions: vec![],
        }
    }
}

impl Default for TriggerInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            events: vec![],
            timing: "BEFORE".to_string(),
            orientation: "STATEMENT".to_string(),
            function_name: String::new(),
            when_clause: None,
        }
    }
}

impl Default for ForeignKeyInfo {
    fn default() -> Self {
        Self {
            constraint_name: String::new(),
            column_name: String::new(),
            foreign_schema: "public".to_string(),
            foreign_table: String::new(),
            foreign_column: String::new(),
            on_delete: "NO ACTION".to_string(),
            on_update: "NO ACTION".to_string(),
        }
    }
}
