//! SQL query constants for database introspection.
//!
//! These queries are used by the Introspector to fetch schema information
//! from PostgreSQL databases via the Supabase API.
//!
//! NOTE: The excluded schemas list in these queries should match the canonical
//! list in `crate::defaults::EXCLUDED_SCHEMAS`. Keep them in sync when modifying.

/// Query to fetch enum types.
pub const ENUMS_QUERY: &str = r#"
    SELECT n.nspname as schema, t.typname as name, array_agg(e.enumlabel ORDER BY e.enumsortorder) as values
    FROM pg_type t
    JOIN pg_enum e ON t.oid = e.enumtypid
    JOIN pg_namespace n ON t.typnamespace = n.oid
    WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
      AND n.nspname NOT LIKE 'pg_toast%'
      AND n.nspname NOT LIKE 'pg_temp%'
      AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
    GROUP BY n.nspname, t.typname
"#;

/// Query to fetch user-defined functions.
pub const FUNCTIONS_QUERY: &str = r#"
    SELECT
      n.nspname as schema,
      p.proname as name,
      pg_get_function_result(p.oid) as return_type,
      pg_get_function_arguments(p.oid) as args,
      l.lanname as language,
      p.prosrc as definition,
      CASE p.provolatile
        WHEN 'i' THEN 'IMMUTABLE'
        WHEN 's' THEN 'STABLE'
        WHEN 'v' THEN 'VOLATILE'
      END as volatility,
      p.proisstrict as is_strict,
      p.prosecdef as security_definer
    FROM pg_proc p
    JOIN pg_language l ON p.prolang = l.oid
    JOIN pg_namespace n ON p.pronamespace = n.oid
    WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
      AND n.nspname NOT LIKE 'pg_toast%'
      AND n.nspname NOT LIKE 'pg_temp%'
      AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
"#;

/// Query to fetch extensions.
pub const EXTENSIONS_QUERY: &str = r#"
    SELECT
        e.extname as name,
        e.extversion as version,
        n.nspname as schema
    FROM pg_extension e
    JOIN pg_namespace n ON n.oid = e.extnamespace
    WHERE e.extname != 'plpgsql'
"#;

/// Query to fetch database roles.
pub const ROLES_QUERY: &str = r#"
    SELECT
        rolname as name,
        rolsuper as superuser,
        rolcreatedb as create_db,
        rolcreaterole as create_role,
        rolinherit as inherit,
        rolcanlogin as login,
        rolreplication as replication,
        rolbypassrls as bypass_rls,
        rolconnlimit as connection_limit,
        rolvaliduntil::text as valid_until
    FROM pg_roles
    WHERE rolname NOT LIKE 'pg_%'
      AND rolname NOT LIKE 'supabase%'
      AND rolname NOT IN ('postgres', 'authenticator', 'authenticated', 'anon', 'service_role', 'dashboard_user', 'pgbouncer')
"#;
