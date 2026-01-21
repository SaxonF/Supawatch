import type { Item, ViewState } from "@/specs/types";
import { type CellBase, type Matrix } from "react-spreadsheet";

export interface CellData extends CellBase {
  value: string;
  readOnly?: boolean;
}

export type SpreadsheetData = Matrix<CellData>;

export interface TableInfo {
  name: string;
  alias: string | null;
  primaryKeyColumn: string | null; // The column name as it appears in results
  primaryKeyField: string; // The actual field name in the table
}

export interface ColumnInfo {
  resultName: string; // Column name as it appears in query results
  tableName: string | null; // Which table this column belongs to
  fieldName: string; // Actual field name in the table
  isComputed: boolean;
  isPrimaryKey: boolean;
}

export interface QueryMetadata {
  tables: TableInfo[];
  columns: ColumnInfo[];
  isEditable: boolean;
}

export interface TableChange {
  tableName: string;
  primaryKeyColumn: string;
  primaryKeyValue: string;
  changes: Record<string, { oldValue: string; newValue: string }>;
}

export interface RowChanges {
  rowIndex: number;
  tableChanges: TableChange[];
}

export interface TableRef {
  schema: string;
  name: string;
}

export interface Tab {
  id: string;
  name: string;
  sql: string;
  results: SpreadsheetData;
  originalResults: SpreadsheetData;
  displayColumns: string[];
  queryMetadata: QueryMetadata | null;
  error: string | null;
  isTableTab: boolean;

  // Spec-driven tab properties
  groupId?: string; // Which group this tab belongs to
  specItem?: Item; // The spec item this tab is based on
  viewStack?: ViewState[]; // Navigation stack within this tab
  formValues?: Record<string, unknown>; // Current form field values
}

export interface SqlEditorProps {
  projectId: string;
}
