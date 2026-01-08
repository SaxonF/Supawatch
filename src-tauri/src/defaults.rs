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
    "plpgsql",  // Always exists in Postgres
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_default_role() {
        assert!(is_default_role("authenticated"));
        assert!(is_default_role("anon"));
        assert!(is_default_role("pg_read_all_data"));
        assert!(is_default_role("supabase_admin"));
        assert!(!is_default_role("my_custom_role"));
    }

    #[test]
    fn test_is_default_extension() {
        assert!(is_default_extension("uuid-ossp"));
        assert!(is_default_extension("pgcrypto"));
        assert!(is_default_extension("pg_graphql"));
        assert!(!is_default_extension("my_custom_extension"));
    }
}
