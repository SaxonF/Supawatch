use crate::schema::{ExtensionInfo, RoleInfo};
use sqlparser::ast::{CreateExtension, CreateRole, Password};
use std::collections::HashMap;

pub fn handle_create_role(
    roles: &mut HashMap<String, RoleInfo>,
    stmt: CreateRole,
) {
    let CreateRole {
        names,
        login,
        inherit,
        superuser,
        create_db,
        create_role,
        replication,
        bypassrls,
        connection_limit,
        password,
        valid_until,
        ..
    } = stmt;

    for name in names {
        let role_name = name.to_string().trim_matches('"').to_string();
        let pwd = match &password {
            Some(Password::Password(p)) => Some(p.to_string().trim_matches('\'').to_string()),
            Some(Password::NullPassword) => None,
            None => None,
        };
        let valid = valid_until.as_ref().map(|v| v.to_string());
        let conn_limit = connection_limit.as_ref()
            .map(|c| c.to_string().parse::<i32>().unwrap_or(-1))
            .unwrap_or(-1);

        roles.insert(
            role_name.clone(),
            RoleInfo {
                name: role_name,
                superuser: superuser.unwrap_or(false),
                create_db: create_db.unwrap_or(false),
                create_role: create_role.unwrap_or(false),
                inherit: inherit.unwrap_or(true),
                login: login.unwrap_or(false),
                replication: replication.unwrap_or(false),
                bypass_rls: bypassrls.unwrap_or(false),
                connection_limit: conn_limit,
                valid_until: valid,
                password: pwd,
            },
        );
    }
}

pub fn handle_create_extension(
    extensions: &mut HashMap<String, ExtensionInfo>,
    stmt: CreateExtension,
) {
    let CreateExtension {
        name,
        schema,
        version,
        ..
    } = stmt;

    let ext_name = name.to_string().trim_matches('"').to_string();
    let ext_schema = schema.map(|s| s.to_string().trim_matches('"').to_string()).unwrap_or("public".to_string());

    extensions.insert(
        ext_name.clone(),
        ExtensionInfo {
            name: ext_name,
            version: version.map(|v| v.to_string().trim_matches('\'').to_string()),
            schema: Some(ext_schema),
        },
    );
}
