use crate::schema::{ForeignKeyInfo, IndexInfo, PolicyInfo, TriggerInfo};

pub fn generate_create_index(table_name: &str, idx: &IndexInfo) -> String {
    let mut sql = if idx.is_unique {
        format!("CREATE UNIQUE INDEX \"{}\"", idx.index_name)
    } else {
        format!("CREATE INDEX \"{}\"", idx.index_name)
    };

    // table_name is already qualified/quoted
    sql.push_str(&format!(" ON {}", table_name));

    // Index method (if not btree)
    if idx.index_method.to_lowercase() != "btree" {
        sql.push_str(&format!(" USING {}", idx.index_method));
    }

    // Columns or expressions
    if !idx.expressions.is_empty() {
        sql.push_str(&format!(" ({})", idx.expressions.join(", ")));
    } else {
        let cols: Vec<String> = idx.columns.iter().map(|c| format!("\"{}\"", c)).collect();
        sql.push_str(&format!(" ({})", cols.join(", ")));
    }

    // WHERE clause for partial indexes
    if let Some(where_clause) = &idx.where_clause {
        sql.push_str(&format!(" WHERE {}", where_clause));
    }

    sql.push(';');
    sql
}

pub fn generate_create_trigger(table_name: &str, trigger: &TriggerInfo) -> String {
    let events = trigger.events.join(" OR ");

    // table_name is already qualified/quoted
    let mut sql = format!(
        "CREATE TRIGGER \"{}\" {} {} ON {} FOR EACH {} ",
        trigger.name, trigger.timing, events, table_name, trigger.orientation
    );

    // WHEN clause
    if let Some(when) = &trigger.when_clause {
        sql.push_str(&format!("WHEN ({}) ", when));
    }

    sql.push_str(&format!("EXECUTE FUNCTION {}();", trigger.function_name));

    sql
}

pub fn generate_create_policy(table_name: &str, policy: &PolicyInfo) -> String {
    // table_name is already qualified/quoted
    let mut sql = format!(
        "CREATE POLICY \"{}\" ON {} FOR {} TO {}",
        policy.name,
        table_name,
        policy.cmd,
        policy.roles.join(", ")
    );

    if let Some(q) = &policy.qual {
        sql.push_str(&format!(" USING ({})", q));
    }

    if let Some(wc) = &policy.with_check {
        sql.push_str(&format!(" WITH CHECK ({})", wc));
    }

    sql.push(';');
    sql
}

pub fn generate_add_foreign_key(table_name: &str, fk: &ForeignKeyInfo) -> String {
    let mut sql = format!(
        "ALTER TABLE {} ADD CONSTRAINT \"{}\" FOREIGN KEY (\"{}\") REFERENCES \"{}\".\"{}\"{}", 
        table_name, fk.constraint_name, fk.column_name, fk.foreign_schema, fk.foreign_table,
        format!("(\"{}\")", fk.foreign_column)
    );

    if fk.on_delete != "NO ACTION" {
        sql.push_str(&format!(" ON DELETE {}", fk.on_delete));
    }

    if fk.on_update != "NO ACTION" {
        sql.push_str(&format!(" ON UPDATE {}", fk.on_update));
    }

    sql.push(';');
    sql
}
