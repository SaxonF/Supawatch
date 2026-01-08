use crate::schema::{ExtensionInfo, RoleInfo};

pub fn generate_create_extension(ext: &ExtensionInfo) -> String {
    let mut sql = format!("CREATE EXTENSION IF NOT EXISTS \"{}\"", ext.name);
    if let Some(schema) = &ext.schema {
        sql.push_str(&format!(" WITH SCHEMA \"{}\"", schema));
    }
    if let Some(version) = &ext.version {
        sql.push_str(&format!(" VERSION '{}'", version));
    }
    sql.push(';');
    sql
}

pub fn generate_create_role(role: &RoleInfo) -> String {
    let mut sql = format!("CREATE ROLE \"{}\"", role.name);

    let mut options = Vec::new();

    if role.superuser { options.push("SUPERUSER"); } else { options.push("NOSUPERUSER"); }
    if role.create_db { options.push("CREATEDB"); } else { options.push("NOCREATEDB"); }
    if role.create_role { options.push("CREATEROLE"); } else { options.push("NOCREATEROLE"); }
    if role.inherit { options.push("INHERIT"); } else { options.push("NOINHERIT"); }
    if role.login { options.push("LOGIN"); } else { options.push("NOLOGIN"); }
    if role.replication { options.push("REPLICATION"); } else { options.push("NOREPLICATION"); }
    if role.bypass_rls { options.push("BYPASSRLS"); } else { options.push("NOBYPASSRLS"); }

    if role.connection_limit != -1 {
         options.push("CONNECTION LIMIT");
    }

    let mut option_str = options.join(" ");

    if role.connection_limit != -1 {
         option_str = option_str.replace("CONNECTION LIMIT", &format!("CONNECTION LIMIT {}", role.connection_limit));
    }

    if let Some(valid) = &role.valid_until {
        option_str.push_str(&format!(" VALID UNTIL '{}'", valid));
    }

    if let Some(pwd) = &role.password {
         option_str.push_str(&format!(" PASSWORD '{}'", pwd));
    }

    if !option_str.is_empty() {
        sql.push_str(" WITH ");
        sql.push_str(&option_str);
    }

    sql.push(';');
    sql
}

pub fn generate_alter_role(role: &RoleInfo) -> String {
    let mut sql = format!("ALTER ROLE \"{}\"", role.name);
    let mut options = Vec::new();

    if role.superuser { options.push("SUPERUSER"); } else { options.push("NOSUPERUSER"); }
    if role.create_db { options.push("CREATEDB"); } else { options.push("NOCREATEDB"); }
    if role.create_role { options.push("CREATEROLE"); } else { options.push("NOCREATEROLE"); }
    if role.inherit { options.push("INHERIT"); } else { options.push("NOINHERIT"); }
    if role.login { options.push("LOGIN"); } else { options.push("NOLOGIN"); }
    if role.replication { options.push("REPLICATION"); } else { options.push("NOREPLICATION"); }
    if role.bypass_rls { options.push("BYPASSRLS"); } else { options.push("NOBYPASSRLS"); }

    if role.connection_limit != -1 {
         options.push("CONNECTION LIMIT");
    }

    let mut option_str = options.join(" ");

    if role.connection_limit != -1 {
         option_str = option_str.replace("CONNECTION LIMIT", &format!("CONNECTION LIMIT {}", role.connection_limit));
    }

    if let Some(valid) = &role.valid_until {
        option_str.push_str(&format!(" VALID UNTIL '{}'", valid));
    }

    if let Some(pwd) = &role.password {
         option_str.push_str(&format!(" PASSWORD '{}'", pwd));
    }

    if !option_str.is_empty() {
        sql.push_str(" WITH ");
        sql.push_str(&option_str);
    }

    sql.push(';');
    sql
}
