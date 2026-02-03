use super::*;
use crate::schema::*;
use crate::diff::tables::compute_table_diff;
use std::collections::HashMap;

#[test]
fn test_column_type_aliases() {
    // reproduction for DECIMAL vs NUMERIC
    let mut local = TableInfo::default();
    local.columns.insert("chance".to_string(), ColumnInfo {
        column_name: "chance".to_string(),
        data_type: "decimal".to_string(), // Local uses alias
        is_nullable: true,
        ..Default::default()
    });

    let mut remote = TableInfo::default();
    remote.columns.insert("chance".to_string(), ColumnInfo {
        column_name: "chance".to_string(),
        data_type: "numeric".to_string(), // Remote uses canonical name
        is_nullable: true,
        ..Default::default()
    });

    // Currently this should produce a diff
    let diff = compute_table_diff(&remote, &local);
    assert!(diff.columns_to_modify.is_empty(), "Should normalize 'decimal' to 'numeric'");
}
