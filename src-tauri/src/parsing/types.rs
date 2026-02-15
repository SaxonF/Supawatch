use crate::schema::{CompositeTypeAttribute, CompositeTypeInfo, DomainCheckConstraint, DomainInfo, EnumInfo};
use sqlparser::ast::{CreateDomain, UserDefinedTypeRepresentation, TableConstraint};
use std::collections::HashMap;
use super::helpers::parse_object_name;

pub fn handle_create_type(
    enums: &mut HashMap<String, EnumInfo>,
    composite_types: &mut HashMap<String, CompositeTypeInfo>,
    name: sqlparser::ast::ObjectName,
    representation: Option<UserDefinedTypeRepresentation>,
) {
    if let Some(rep) = representation {
        match rep {
            UserDefinedTypeRepresentation::Enum { labels, .. } => {
                let (schema, enum_name) = parse_object_name(&name);
                let key = format!("\"{}\".\"{}\"", schema, enum_name);
                enums.insert(
                    key,
                    EnumInfo {
                        schema,
                        name: enum_name,
                        values: labels.iter().map(|v| v.value.clone()).collect(),
                        extension: None,
                    },
                );
            }
            UserDefinedTypeRepresentation::Composite { attributes } => {
                let (schema, type_name) = parse_object_name(&name);
                let attrs: Vec<CompositeTypeAttribute> = attributes
                    .iter()
                    .map(|attr| CompositeTypeAttribute {
                        name: attr.name.to_string(),
                        data_type: attr.data_type.to_string(),
                        collation: attr.collation.as_ref().map(|c| c.to_string()),
                    })
                    .collect();

                let key = format!("\"{}\".\"{}\"", schema, type_name);
                composite_types.insert(
                    key,
                    CompositeTypeInfo {
                        schema,
                        name: type_name,
                        attributes: attrs,
                        comment: None,
                        extension: None,
                    },
                );
            }
            _ => {}
        }
    }
}

pub fn handle_create_domain(
    domains: &mut HashMap<String, DomainInfo>,
    stmt: CreateDomain,
) {
    let CreateDomain {
        name,
        data_type,
        default,
        constraints,
        collation,
    } = stmt;

    let (schema, domain_name) = parse_object_name(&name);
    let base_type = data_type.to_string().to_lowercase();
    let default_value = default.map(|d| d.to_string());

    let is_not_null = false; // Default to false
    let mut check_constraints = vec![];

    for constraint in constraints {
        match constraint {
            TableConstraint::Check(chk) => {
                check_constraints.push(DomainCheckConstraint {
                    name: chk.name.as_ref().map(|n| n.value.clone()),
                    expression: format!("CHECK ({})", chk.expr),
                });
            }
            _ => {}
        }
    }

    let key = format!("\"{}\".\"{}\"", schema, domain_name);
    domains.insert(
        key,
        DomainInfo {
            schema,
            name: domain_name,
            base_type,
            default_value,
            is_not_null,
            check_constraints,
            collation: collation.map(|c| c.to_string()),
            comment: None,
            extension: None,
        },
    );
}
