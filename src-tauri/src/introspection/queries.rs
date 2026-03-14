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

pub const SCHEMA_GRANTS_QUERY: &str = r#"
    SELECT 
        n.nspname AS schema,
        COALESCE(r.rolname, 'public') AS grantee,
        acl.privilege_type AS privilege
    FROM pg_namespace n
    CROSS JOIN LATERAL aclexplode(COALESCE(n.nspacl, acldefault('n', n.nspowner))) acl
    LEFT JOIN pg_roles r ON r.oid = acl.grantee
    WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
      AND n.nspname NOT LIKE 'pg_toast%'
      AND n.nspname NOT LIKE 'pg_temp%'
"#;

pub const OBJECT_GRANTS_QUERY: &str = r#"
    SELECT n.nspname AS schema, c.relname AS object_name,
           CASE c.relkind WHEN 'r' THEN 'table' WHEN 'v' THEN 'view' WHEN 'm' THEN 'view' WHEN 'S' THEN 'sequence' END AS object_type,
           COALESCE(r.rolname, 'public') AS grantee,
           acl.privilege_type AS privilege
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    CROSS JOIN LATERAL aclexplode(COALESCE(c.relacl, acldefault((CASE WHEN c.relkind = 'S' THEN 's' ELSE 'r' END)::"char", c.relowner))) acl
    LEFT JOIN pg_roles r ON r.oid = acl.grantee
    WHERE c.relkind IN ('r', 'v', 'm', 'S')
      AND n.nspname NOT IN ('pg_catalog', 'information_schema')
      AND n.nspname NOT LIKE 'pg_toast%'
      AND n.nspname NOT LIKE 'pg_temp%'
      AND n.nspname NOT IN ('auth', 'storage', 'extensions', 'realtime', 'graphql', 'graphql_public', 'vault', 'pgsodium', 'pgsodium_masks', 'supa_audit', 'net', 'pgtle', 'repack', 'tiger', 'topology', 'supabase_migrations', 'supabase_functions', 'cron', 'pgbouncer')
"#;

pub const DEFAULT_PRIVILEGES_QUERY: &str = r#"
    -- We aggregate table privileges across the schema to simulate "ALL TABLES IN SCHEMA" presence.
    -- If a role has a privilege on ALL tables in a schema, we consider it a schema-wide "ALL TABLES" grant.
    WITH schema_tables AS (
        SELECT n.nspname, c.oid, c.relname
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relkind IN ('r', 'v', 'p') 
          AND n.nspname NOT IN ('pg_catalog', 'information_schema')
          AND n.nspname NOT LIKE 'pg_toast%'
          AND n.nspname NOT LIKE 'pg_temp%'
    ),
    table_grants AS (
        SELECT 
            st.nspname AS schema,
            COALESCE(r.rolname, 'public') AS grantee,
            acl.privilege_type AS privilege,
            st.oid
        FROM schema_tables st
        CROSS JOIN LATERAL aclexplode(COALESCE((SELECT relacl FROM pg_class WHERE oid = st.oid), acldefault('r', (SELECT relowner FROM pg_class WHERE oid = st.oid)))) acl
        LEFT JOIN pg_roles r ON r.oid = acl.grantee
    ),
    schema_counts AS (
        SELECT nspname AS schema, count(*) AS total_tables FROM schema_tables GROUP BY nspname
    ),
    grant_counts AS (
        SELECT schema, grantee, privilege, count(*) AS granted_tables
        FROM table_grants
        GROUP BY schema, grantee, privilege
    )
    SELECT gc.schema, 'tables' AS object_type, gc.grantee, gc.privilege
    FROM grant_counts gc
    JOIN schema_counts sc ON sc.schema = gc.schema
    WHERE gc.granted_tables = sc.total_tables
      AND sc.total_tables > 0
"#;
