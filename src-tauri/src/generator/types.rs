use crate::schema::{CompositeTypeInfo, DomainInfo};

pub fn generate_create_domain(domain: &DomainInfo) -> String {
    let mut sql = format!("CREATE DOMAIN \"{}\".\"{}\" AS {}", domain.schema, domain.name, domain.base_type);

    if let Some(collation) = &domain.collation {
        sql.push_str(&format!(" COLLATE \"{}\"", collation));
    }

    if let Some(default) = &domain.default_value {
        sql.push_str(&format!(" DEFAULT {}", default));
    }

    if domain.is_not_null {
        sql.push_str(" NOT NULL");
    }

    for check in &domain.check_constraints {
        if let Some(name) = &check.name {
            sql.push_str(&format!(" CONSTRAINT \"{}\"", name));
        }
        sql.push_str(&format!(" {}", check.expression));
    }

    sql.push(';');
    sql
}

pub fn generate_create_composite_type(comp_type: &CompositeTypeInfo) -> String {
    let attrs: Vec<String> = comp_type
        .attributes
        .iter()
        .map(|a| {
            let mut attr_sql = format!("\"{}\" {}", a.name, a.data_type);
            if let Some(collation) = &a.collation {
                attr_sql.push_str(&format!(" COLLATE \"{}\"", collation));
            }
            attr_sql
        })
        .collect();

    format!(
        "CREATE TYPE \"{}\".\"{}\" AS (\n  {}\n);",
        comp_type.schema,
        comp_type.name,
        attrs.join(",\n  ")
    )
}

pub fn generate_create_enum(name: &str, values: &[String]) -> String {
    let quoted_values: Vec<String> = values.iter().map(|v| format!("'{}'", v)).collect();
    // Name is already qualified
    format!(
        "CREATE TYPE {} AS ENUM ({});",
        name,
        quoted_values.join(", ")
    )
}
