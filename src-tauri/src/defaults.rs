//! Default Supabase objects that should be ignored during schema diffing.
//!
//! These are system-managed objects that exist in every Supabase project
//! and should not be included in schema migrations.

/// Default roles that exist in every Supabase project.
/// These should be excluded from diff operations to prevent
/// attempting to drop or create these system-managed roles.
pub const DEFAULT_ROLES: &[&str] = &[
    "authenticated",
    "anon",
    "service_role",
    "authenticator",
    "postgres",
    "dashboard_user",
    "pgbouncer",
    "cli_login_postgres",
    // Note: pg_* and supabase* roles are already filtered in queries.rs
];

/// Default extensions that exist in every Supabase project.
/// These should be excluded from diff operations to prevent
/// attempting to drop these system-managed extensions.
pub const DEFAULT_EXTENSIONS: &[&str] = &[
    "uuid-ossp",
    "supabase_vault",
    "pgcrypto",
    "pg_graphql",
    "pg_stat_statements",
    "pgjwt",
    "pgsodium",
    "plpgsql", // Always exists in Postgres
];

/// System schemas managed by PostgreSQL and Supabase.
/// These should be excluded from introspection and generation to prevent
/// interfering with system-managed database objects.
pub const EXCLUDED_SCHEMAS: &[&str] = &[
    // PostgreSQL system schemas
    "pg_catalog",
    "information_schema",
    // Supabase internal schemas
    "auth",
    "storage",
    "extensions",
    "realtime",
    "graphql",
    "graphql_public",
    "vault",
    "pgsodium",
    "pgsodium_masks",
    "supa_audit",
    "net",
    "pgtle",
    "repack",
    "tiger",
    "topology",
    "supabase_migrations",
    "supabase_functions",
    "cron",
    "pgbouncer",
];

/// Check if a role name is a default Supabase role.
pub fn is_default_role(name: &str) -> bool {
    DEFAULT_ROLES.contains(&name)
        || name.starts_with("pg_")
        || name.starts_with("supabase")
}

/// Check if an extension name is a default Supabase extension.
pub fn is_default_extension(name: &str) -> bool {
    DEFAULT_EXTENSIONS.contains(&name)
}

/// Check if a schema name is a system/excluded schema.
pub fn is_excluded_schema(name: &str) -> bool {
    EXCLUDED_SCHEMAS.contains(&name)
        || name.starts_with("pg_")
        || name == "public" // public is a special case - exists by default
}

/// Generate a SQL-formatted list of excluded schemas for use in queries.
/// Returns format: 'auth', 'storage', 'extensions', ...
pub fn excluded_schemas_sql_list() -> String {
    EXCLUDED_SCHEMAS
        .iter()
        .map(|s| format!("'{}'", s))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_default_role() {
        assert!(is_default_role("authenticated"));
        assert!(is_default_role("anon"));
        assert!(is_default_role("pg_read_all_data"));
        assert!(is_default_role("supabase_admin"));
        assert!(is_default_role("cli_login_postgres"));
        assert!(!is_default_role("my_custom_role"));
    }

    #[test]
    fn test_is_default_extension() {
        assert!(is_default_extension("uuid-ossp"));
        assert!(is_default_extension("pgcrypto"));
        assert!(is_default_extension("pg_graphql"));
        assert!(!is_default_extension("my_custom_extension"));
    }

    #[test]
    fn test_is_excluded_schema() {
        // System schemas
        assert!(is_excluded_schema("auth"));
        assert!(is_excluded_schema("storage"));
        assert!(is_excluded_schema("pg_catalog"));
        assert!(is_excluded_schema("information_schema"));
        assert!(is_excluded_schema("realtime"));
        assert!(is_excluded_schema("supabase_migrations"));
        // pg_ prefix patterns
        assert!(is_excluded_schema("pg_toast"));
        assert!(is_excluded_schema("pg_temp_1"));
        // public is special (always exists)
        assert!(is_excluded_schema("public"));
        // Custom schemas should NOT be excluded
        assert!(!is_excluded_schema("my_custom_schema"));
        assert!(!is_excluded_schema("app"));
    }

    #[test]
    fn test_excluded_schemas_sql_list() {
        let sql_list = excluded_schemas_sql_list();
        assert!(sql_list.contains("'auth'"));
        assert!(sql_list.contains("'storage'"));
        assert!(sql_list.contains("'extensions'"));
        // Should be comma-separated
        assert!(sql_list.contains(", "));
    }
}
