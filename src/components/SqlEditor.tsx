import { Play, Save } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import Spreadsheet, { type CellBase, type Matrix } from "react-spreadsheet";
import * as api from "../api";
import { Button } from "./ui/button";

interface SqlEditorProps {
  projectId: string;
}

interface CellData extends CellBase {
  value: string;
  readOnly?: boolean;
}

type SpreadsheetData = Matrix<CellData>;

interface TableInfo {
  name: string;
  alias: string | null;
  primaryKeyColumn: string | null; // The column name as it appears in results
  primaryKeyField: string; // The actual field name in the table
}

interface ColumnInfo {
  resultName: string; // Column name as it appears in query results
  tableName: string | null; // Which table this column belongs to
  fieldName: string; // Actual field name in the table
  isComputed: boolean;
  isPrimaryKey: boolean;
}

interface QueryMetadata {
  tables: TableInfo[];
  columns: ColumnInfo[];
  isEditable: boolean;
}

interface TableChange {
  tableName: string;
  primaryKeyColumn: string;
  primaryKeyValue: string;
  changes: Record<string, { oldValue: string; newValue: string }>;
}

interface RowChanges {
  rowIndex: number;
  tableChanges: TableChange[];
}

// Parse tables from FROM and JOIN clauses
function parseTables(sql: string): TableInfo[] {
  const normalized = sql.replace(/\s+/g, " ").trim();
  const tables: TableInfo[] = [];

  // Match FROM table [alias]
  const fromMatch = normalized.match(/\bfrom\s+([a-z_][a-z0-9_]*)(?:\s+(?:as\s+)?([a-z_][a-z0-9_]*))?/i);
  if (fromMatch) {
    tables.push({
      name: fromMatch[1].toLowerCase(),
      alias: fromMatch[2]?.toLowerCase() || null,
      primaryKeyColumn: null,
      primaryKeyField: "id",
    });
  }

  // Match JOIN table [alias]
  const joinRegex = /\bjoin\s+([a-z_][a-z0-9_]*)(?:\s+(?:as\s+)?([a-z_][a-z0-9_]*))?/gi;
  let joinMatch;
  while ((joinMatch = joinRegex.exec(normalized)) !== null) {
    tables.push({
      name: joinMatch[1].toLowerCase(),
      alias: joinMatch[2]?.toLowerCase() || null,
      primaryKeyColumn: null,
      primaryKeyField: "id",
    });
  }

  return tables;
}

// Check if query has non-editable constructs (aggregations, etc.)
function hasNonEditableConstructs(sql: string): boolean {
  const normalized = sql.replace(/\s+/g, " ").trim().toLowerCase();

  const nonEditablePatterns = [
    /\bgroup\s+by\b/,
    /\bhaving\b/,
    /\bunion\b/,
    /\bintersect\b/,
    /\bexcept\b/,
    /\bdistinct\b/,
    /\bcount\s*\(/,
    /\bsum\s*\(/,
    /\bavg\s*\(/,
    /\bmin\s*\(/,
    /\bmax\s*\(/,
  ];

  return nonEditablePatterns.some((pattern) => pattern.test(normalized));
}

// Parse column info from SELECT clause and result columns
function parseColumns(
  sql: string,
  resultColumns: string[],
  tables: TableInfo[]
): ColumnInfo[] {
  const normalized = sql.replace(/\s+/g, " ").trim();

  // Extract SELECT clause
  const selectMatch = normalized.match(/select\s+(.+?)\s+from\s/i);
  if (!selectMatch) return resultColumns.map((col) => ({
    resultName: col,
    tableName: null,
    fieldName: col,
    isComputed: false,
    isPrimaryKey: false,
  }));

  const selectClause = selectMatch[1];
  const isSelectStar = selectClause.trim() === "*";

  // Build alias to table name map
  const aliasMap: Record<string, string> = {};
  for (const table of tables) {
    if (table.alias) {
      aliasMap[table.alias] = table.name;
    }
    aliasMap[table.name] = table.name;
  }

  return resultColumns.map((resultCol) => {
    const info: ColumnInfo = {
      resultName: resultCol,
      tableName: null,
      fieldName: resultCol,
      isComputed: false,
      isPrimaryKey: false,
    };

    // Check if column name contains table prefix (e.g., "users.name" or result is "users_name")
    // First, try to find explicit prefix in SELECT clause
    if (!isSelectStar) {
      // Look for patterns like: table.column as alias, table.column alias, table.column
      const prefixPattern = new RegExp(
        `\\b([a-z_][a-z0-9_]*)\\.([a-z_][a-z0-9_]*)(?:\\s+(?:as\\s+)?${resultCol})?\\b`,
        "gi"
      );

      let match;
      while ((match = prefixPattern.exec(selectClause)) !== null) {
        const [fullMatch, prefix, field] = match;
        // Check if this matches our result column
        if (
          fullMatch.toLowerCase().includes(resultCol.toLowerCase()) ||
          field.toLowerCase() === resultCol.toLowerCase()
        ) {
          const tableName = aliasMap[prefix.toLowerCase()];
          if (tableName) {
            info.tableName = tableName;
            info.fieldName = field;
            break;
          }
        }
      }
    }

    // For SELECT *, try to infer table from column naming conventions
    if (!info.tableName && tables.length === 1) {
      // Single table query - all columns belong to that table
      info.tableName = tables[0].name;
    }

    // Check if this is a computed column
    if (!isSelectStar) {
      const computedPatterns = [
        new RegExp(`\\([^)]+\\)\\s+(?:as\\s+)?${resultCol}\\b`, "i"),
        new RegExp(`\\w+\\s*[+\\-*/]\\s*\\w+.*?(?:as\\s+)?${resultCol}\\b`, "i"),
        new RegExp(`\\w+\\s*\\|\\|\\s*\\w+.*?(?:as\\s+)?${resultCol}\\b`, "i"),
        new RegExp(`\\b(?:coalesce|case|nullif|concat)\\s*\\(.*?(?:as\\s+)?${resultCol}\\b`, "i"),
      ];

      info.isComputed = computedPatterns.some((p) => p.test(selectClause));
    }

    return info;
  });
}

// Find primary key columns for each table in the result set
function findPrimaryKeys(
  columns: ColumnInfo[],
  tables: TableInfo[]
): void {
  const pkNames = ["id", "uuid", "pk", "_id"];

  for (const table of tables) {
    // Look for table-prefixed primary key first (e.g., users.id -> users_id or just id if single table)
    for (const pkName of pkNames) {
      const matchingCol = columns.find(
        (col) =>
          col.tableName === table.name &&
          col.fieldName.toLowerCase() === pkName
      );
      if (matchingCol) {
        table.primaryKeyColumn = matchingCol.resultName;
        table.primaryKeyField = pkName;
        matchingCol.isPrimaryKey = true;
        break;
      }
    }

    // If no prefixed PK found, look for standalone pk columns
    if (!table.primaryKeyColumn) {
      for (const pkName of pkNames) {
        const matchingCol = columns.find(
          (col) => col.resultName.toLowerCase() === pkName && !col.isPrimaryKey
        );
        if (matchingCol) {
          table.primaryKeyColumn = matchingCol.resultName;
          table.primaryKeyField = pkName;
          matchingCol.isPrimaryKey = true;
          // Assign this column to the table if not assigned
          if (!matchingCol.tableName) {
            matchingCol.tableName = table.name;
          }
          break;
        }
      }
    }
  }
}

// Format a value for display in the spreadsheet
function formatCellValue(value: unknown): string {
  if (value === null) return "NULL";
  if (typeof value === "object") {
    return JSON.stringify(value);
  }
  return String(value);
}

// Check if a string looks like JSON
function isJsonString(str: string): boolean {
  if (str === "NULL") return false;
  const trimmed = str.trim();
  return (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  );
}

// Generate UPDATE SQL for a table
function generateUpdateSql(
  tableName: string,
  primaryKeyField: string,
  primaryKeyValue: string,
  changes: Record<string, { oldValue: string; newValue: string }>
): string {
  const setClauses = Object.entries(changes)
    .map(([fieldName, { newValue }]) => {
      if (newValue === "NULL") {
        return `"${fieldName}" = NULL`;
      }

      const escapedValue = `'${newValue.replace(/'/g, "''")}'`;

      if (isJsonString(newValue)) {
        return `"${fieldName}" = ${escapedValue}::jsonb`;
      }

      return `"${fieldName}" = ${escapedValue}`;
    })
    .join(", ");

  const escapedPkValue = `'${primaryKeyValue.replace(/'/g, "''")}'`;

  return `UPDATE "${tableName}" SET ${setClauses} WHERE "${primaryKeyField}" = ${escapedPkValue}`;
}

export function SqlEditor({ projectId }: SqlEditorProps) {
  const [sql, setSql] = useState("SELECT * FROM ");
  const [results, setResults] = useState<SpreadsheetData>([]);
  const [originalResults, setOriginalResults] = useState<SpreadsheetData>([]);
  const [displayColumns, setDisplayColumns] = useState<string[]>([]);
  const [queryMetadata, setQueryMetadata] = useState<QueryMetadata | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Calculate changes between original and current results, grouped by table
  const changes = useMemo((): RowChanges[] => {
    if (!queryMetadata?.isEditable) return [];

    const rowChanges: RowChanges[] = [];

    for (let rowIdx = 0; rowIdx < results.length; rowIdx++) {
      const currentRow = results[rowIdx];
      const originalRow = originalResults[rowIdx];
      if (!currentRow || !originalRow) continue;

      // Group changes by table
      const tableChangesMap: Record<string, TableChange> = {};

      for (let colIdx = 0; colIdx < queryMetadata.columns.length; colIdx++) {
        const colInfo = queryMetadata.columns[colIdx];
        const currentCell = currentRow[colIdx];
        const originalCell = originalRow[colIdx];

        // Skip readonly, computed, primary key, or unassigned columns
        if (currentCell?.readOnly) continue;
        if (colInfo.isComputed || colInfo.isPrimaryKey || !colInfo.tableName) continue;

        const currentValue = currentCell?.value ?? "";
        const originalValue = originalCell?.value ?? "";

        if (currentValue !== originalValue) {
          // Find the table info
          const tableInfo = queryMetadata.tables.find((t) => t.name === colInfo.tableName);
          if (!tableInfo || !tableInfo.primaryKeyColumn) continue;

          // Get primary key value for this table
          const pkColIdx = queryMetadata.columns.findIndex(
            (c) => c.resultName === tableInfo.primaryKeyColumn
          );
          if (pkColIdx === -1) continue;

          const pkValue = currentRow[pkColIdx]?.value;
          if (!pkValue) continue;

          // Initialize table change entry if needed
          const tableKey = `${tableInfo.name}:${pkValue}`;
          if (!tableChangesMap[tableKey]) {
            tableChangesMap[tableKey] = {
              tableName: tableInfo.name,
              primaryKeyColumn: tableInfo.primaryKeyField,
              primaryKeyValue: pkValue,
              changes: {},
            };
          }

          tableChangesMap[tableKey].changes[colInfo.fieldName] = {
            oldValue: originalValue,
            newValue: currentValue,
          };
        }
      }

      const tableChanges = Object.values(tableChangesMap);
      if (tableChanges.length > 0) {
        rowChanges.push({ rowIndex: rowIdx, tableChanges });
      }
    }

    return rowChanges;
  }, [results, originalResults, queryMetadata]);

  // Count total changes
  const changesSummary = useMemo(() => {
    let totalChanges = 0;
    const tablesAffected = new Set<string>();

    for (const row of changes) {
      for (const tc of row.tableChanges) {
        totalChanges += Object.keys(tc.changes).length;
        tablesAffected.add(tc.tableName);
      }
    }

    return { totalChanges, rowCount: changes.length, tableCount: tablesAffected.size };
  }, [changes]);

  const runQuery = async () => {
    if (!sql.trim()) return;

    setIsLoading(true);
    setError(null);

    try {
      const result = await api.runQuery(projectId, sql, true);

      if (Array.isArray(result) && result.length > 0) {
        const resultCols = Object.keys(result[0]);
        setDisplayColumns(resultCols);

        // Parse query structure
        const tables = parseTables(sql);
        const hasNonEditable = hasNonEditableConstructs(sql);
        const columns = parseColumns(sql, resultCols, tables);

        // Find primary keys for each table
        findPrimaryKeys(columns, tables);

        // Determine if query is editable
        const isEditable =
          !hasNonEditable &&
          tables.length > 0 &&
          tables.some((t) => t.primaryKeyColumn !== null);

        const metadata: QueryMetadata = {
          tables,
          columns,
          isEditable,
        };

        setQueryMetadata(metadata);

        // Convert to spreadsheet format with readonly flags
        const data: CellData[][] = result.map((row: Record<string, unknown>) =>
          columns.map((colInfo) => {
            const value = row[colInfo.resultName];
            const table = tables.find((t) => t.name === colInfo.tableName);
            const hasTablePk = table?.primaryKeyColumn !== null;

            return {
              value: formatCellValue(value),
              readOnly:
                !isEditable ||
                colInfo.isComputed ||
                colInfo.isPrimaryKey ||
                !colInfo.tableName ||
                !hasTablePk,
            };
          })
        );

        setResults(data);
        setOriginalResults(JSON.parse(JSON.stringify(data)));
      } else {
        setDisplayColumns([]);
        setResults([]);
        setOriginalResults([]);
        setQueryMetadata(null);
      }
    } catch (err) {
      console.error("Query failed:", err);
      setError(typeof err === "string" ? err : String(err));
      setResults([]);
      setOriginalResults([]);
      setDisplayColumns([]);
      setQueryMetadata(null);
    } finally {
      setIsLoading(false);
    }
  };

  const saveChanges = async () => {
    if (!queryMetadata?.isEditable || changes.length === 0) return;

    setIsSaving(true);
    setError(null);

    try {
      // Generate and execute UPDATE statements for each table change
      for (const rowChange of changes) {
        for (const tableChange of rowChange.tableChanges) {
          const updateSql = generateUpdateSql(
            tableChange.tableName,
            tableChange.primaryKeyColumn,
            tableChange.primaryKeyValue,
            tableChange.changes
          );

          await api.runQuery(projectId, updateSql, false);
        }
      }

      // Update original results to reflect saved state
      setOriginalResults(JSON.parse(JSON.stringify(results)));
    } catch (err) {
      console.error("Save failed:", err);
      setError(typeof err === "string" ? err : String(err));
    } finally {
      setIsSaving(false);
    }
  };

  const handleDataChange = useCallback((newData: SpreadsheetData) => {
    setResults(newData);
  }, []);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      runQuery();
    }
  };

  const hasChanges = changesSummary.totalChanges > 0;

  // Build editable tables summary
  const editableTables = queryMetadata?.tables
    .filter((t) => t.primaryKeyColumn !== null)
    .map((t) => t.name) || [];

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* SQL Input Area */}
      <div className="shrink-0 p-4 border-b">
        <div className="flex gap-2">
          <textarea
            value={sql}
            onChange={(e) => setSql(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="SELECT * FROM your_table"
            className="flex-1 min-h-[100px] p-3 bg-muted rounded-lg border border-input font-mono text-sm resize-y focus:outline-none focus:ring-2 focus:ring-ring"
            spellCheck={false}
          />
          <Button
            onClick={runQuery}
            disabled={isLoading || !sql.trim()}
            className="shrink-0 self-start"
            title="Run query (Cmd+Enter)"
          >
            <Play size={16} className="mr-2" />
            {isLoading ? "Running..." : "Run"}
          </Button>
        </div>
        <p className="text-xs text-muted-foreground mt-2">
          Press Cmd+Enter to run query
          {queryMetadata?.isEditable && editableTables.length > 0 && (
            <span className="ml-2 text-green-500">
              • Editable: {editableTables.join(", ")}
            </span>
          )}
          {queryMetadata && !queryMetadata.isEditable && (
            <span className="ml-2 text-yellow-500">• Read-only</span>
          )}
        </p>
      </div>

      {/* Results Area */}
      <div className="flex-1 overflow-auto p-4">
        {error ? (
          <div className="p-4 bg-destructive/10 rounded-lg border border-destructive/20">
            <p className="text-destructive text-sm font-mono whitespace-pre-wrap">
              {error}
            </p>
          </div>
        ) : results.length > 0 ? (
          <div className="sql-results-spreadsheet">
            <Spreadsheet
              data={results}
              columnLabels={displayColumns}
              onChange={handleDataChange}
            />
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
            <p>No results</p>
            <p className="text-sm mt-1">Run a query to see results here</p>
          </div>
        )}
      </div>

      {/* Changes Bar */}
      {hasChanges && (
        <div className="shrink-0 px-4 py-3 border-t bg-muted/50 flex items-center justify-between">
          <span className="text-sm text-muted-foreground">
            {changesSummary.totalChanges} change{changesSummary.totalChanges !== 1 ? "s" : ""} to{" "}
            {changesSummary.rowCount} row{changesSummary.rowCount !== 1 ? "s" : ""}
            {changesSummary.tableCount > 1 && ` across ${changesSummary.tableCount} tables`}
          </span>
          <Button
            onClick={saveChanges}
            disabled={isSaving}
            size="sm"
          >
            <Save size={14} className="mr-2" />
            {isSaving ? "Saving..." : "Save"}
          </Button>
        </div>
      )}
    </div>
  );
}
