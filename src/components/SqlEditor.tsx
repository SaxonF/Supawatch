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

interface QueryMetadata {
  tableName: string | null;
  isEditable: boolean;
  primaryKeyColumn: string | null;
  columns: string[];
}

interface RowChange {
  rowIndex: number;
  primaryKeyValue: string;
  changes: Record<string, { oldValue: string; newValue: string }>;
}

// Parse a SELECT query to extract metadata for editing
function parseSelectQuery(sql: string): QueryMetadata {
  const normalized = sql.replace(/\s+/g, " ").trim().toLowerCase();

  // Default: not editable
  const result: QueryMetadata = {
    tableName: null,
    isEditable: false,
    primaryKeyColumn: null,
    columns: [],
  };

  // Check for things that make a query non-editable
  const nonEditablePatterns = [
    /\bjoin\b/,
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
    /\bcoalesce\s*\(/,
    /\bcase\s+when\b/,
  ];

  for (const pattern of nonEditablePatterns) {
    if (pattern.test(normalized)) {
      return result;
    }
  }

  // Try to extract table name from simple SELECT ... FROM table
  const fromMatch = normalized.match(/\bfrom\s+([a-z_][a-z0-9_]*)/);
  if (!fromMatch) {
    return result;
  }

  result.tableName = fromMatch[1];
  result.isEditable = true;

  return result;
}

// Detect which columns are likely computed/expressions vs simple columns
function isComputedColumn(columnName: string, sql: string): boolean {
  const normalized = sql.replace(/\s+/g, " ").trim();

  // Extract the SELECT clause
  const selectMatch = normalized.match(/select\s+(.+?)\s+from\s/i);
  if (!selectMatch) return false;

  const selectClause = selectMatch[1];

  // If SELECT *, all columns are from the table
  if (selectClause.trim() === "*") {
    return false;
  }

  // Check if this column appears as an alias for an expression
  const expressionPatterns = [
    new RegExp(`\\([^)]+\\)\\s+(?:as\\s+)?${columnName}`, "i"), // (expr) as col
    new RegExp(`\\w+\\s*[+\\-*/]\\s*\\w+.*?(?:as\\s+)?${columnName}`, "i"), // a + b as col
    new RegExp(`\\w+\\s*\\|\\|\\s*\\w+.*?(?:as\\s+)?${columnName}`, "i"), // a || b as col
  ];

  for (const pattern of expressionPatterns) {
    if (pattern.test(selectClause)) {
      return true;
    }
  }

  return false;
}

// Find the primary key column (prefer 'id', then first column)
function findPrimaryKeyColumn(columns: string[]): string | null {
  if (columns.length === 0) return null;

  // Common primary key names
  const pkNames = ["id", "uuid", "pk", "_id"];
  for (const pk of pkNames) {
    if (columns.includes(pk)) return pk;
  }

  // Columns ending with _id might be primary keys
  const idColumn = columns.find((c) => c.endsWith("_id") && c !== "created_by_id");
  if (idColumn) return idColumn;

  // Fall back to first column (often the primary key)
  return columns[0];
}

// Format a value for display in the spreadsheet
function formatCellValue(value: unknown): string {
  if (value === null) return "NULL";
  if (typeof value === "object") {
    // JSON/JSONB columns - stringify for display and editing
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

// Generate UPDATE SQL for a set of changes
function generateUpdateSql(
  tableName: string,
  primaryKeyColumn: string,
  primaryKeyValue: string,
  changes: Record<string, { oldValue: string; newValue: string }>
): string {
  const setClauses = Object.entries(changes)
    .map(([column, { newValue }]) => {
      if (newValue === "NULL") {
        return `"${column}" = NULL`;
      }

      const escapedValue = `'${newValue.replace(/'/g, "''")}'`;

      // If the value looks like JSON, cast it to jsonb
      if (isJsonString(newValue)) {
        return `"${column}" = ${escapedValue}::jsonb`;
      }

      return `"${column}" = ${escapedValue}`;
    })
    .join(", ");

  const escapedPkValue = `'${primaryKeyValue.replace(/'/g, "''")}'`;

  return `UPDATE "${tableName}" SET ${setClauses} WHERE "${primaryKeyColumn}" = ${escapedPkValue}`;
}

export function SqlEditor({ projectId }: SqlEditorProps) {
  const [sql, setSql] = useState("SELECT * FROM ");
  const [results, setResults] = useState<SpreadsheetData>([]);
  const [originalResults, setOriginalResults] = useState<SpreadsheetData>([]);
  const [columns, setColumns] = useState<string[]>([]);
  const [queryMetadata, setQueryMetadata] = useState<QueryMetadata | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Calculate changes between original and current results
  const changes = useMemo((): RowChange[] => {
    if (!queryMetadata?.isEditable || !queryMetadata.primaryKeyColumn) {
      return [];
    }

    const pkIndex = columns.indexOf(queryMetadata.primaryKeyColumn);
    if (pkIndex === -1) return [];

    const rowChanges: RowChange[] = [];

    for (let rowIdx = 0; rowIdx < results.length; rowIdx++) {
      const currentRow = results[rowIdx];
      const originalRow = originalResults[rowIdx];

      if (!currentRow || !originalRow) continue;

      const pkValue = currentRow[pkIndex]?.value;
      if (!pkValue) continue;

      const changesInRow: Record<string, { oldValue: string; newValue: string }> = {};

      for (let colIdx = 0; colIdx < columns.length; colIdx++) {
        const col = columns[colIdx];
        const currentCell = currentRow[colIdx];
        const originalCell = originalRow[colIdx];

        // Skip readonly columns and primary key
        if (currentCell?.readOnly) continue;
        if (col === queryMetadata.primaryKeyColumn) continue;

        const currentValue = currentCell?.value ?? "";
        const originalValue = originalCell?.value ?? "";

        if (currentValue !== originalValue) {
          changesInRow[col] = { oldValue: originalValue, newValue: currentValue };
        }
      }

      if (Object.keys(changesInRow).length > 0) {
        rowChanges.push({
          rowIndex: rowIdx,
          primaryKeyValue: pkValue,
          changes: changesInRow,
        });
      }
    }

    return rowChanges;
  }, [results, originalResults, columns, queryMetadata]);

  // Count total changes
  const changesSummary = useMemo(() => {
    const totalChanges = changes.reduce(
      (sum, row) => sum + Object.keys(row.changes).length,
      0
    );
    const rowCount = changes.length;
    return { totalChanges, rowCount };
  }, [changes]);

  const runQuery = async () => {
    if (!sql.trim()) return;

    setIsLoading(true);
    setError(null);

    try {
      const result = await api.runQuery(projectId, sql, true);
      const metadata = parseSelectQuery(sql);

      if (Array.isArray(result) && result.length > 0) {
        const cols = Object.keys(result[0]);
        setColumns(cols);

        // Find primary key column
        metadata.primaryKeyColumn = findPrimaryKeyColumn(cols);
        metadata.columns = cols;

        // If no primary key found in results, mark as non-editable
        if (!metadata.primaryKeyColumn || !cols.includes(metadata.primaryKeyColumn)) {
          metadata.isEditable = false;
        }

        setQueryMetadata(metadata);

        // Convert to spreadsheet format with readonly flags
        const data: CellData[][] = result.map((row: Record<string, unknown>) =>
          cols.map((col) => {
            const value = row[col];
            const isComputed = isComputedColumn(col, sql);
            const isPrimaryKey = col === metadata.primaryKeyColumn;

            return {
              value: formatCellValue(value),
              readOnly: !metadata.isEditable || isComputed || isPrimaryKey,
            };
          })
        );

        setResults(data);
        setOriginalResults(JSON.parse(JSON.stringify(data)));
      } else {
        setColumns([]);
        setResults([]);
        setOriginalResults([]);
        setQueryMetadata(null);
      }
    } catch (err) {
      console.error("Query failed:", err);
      setError(typeof err === "string" ? err : String(err));
      setResults([]);
      setOriginalResults([]);
      setColumns([]);
      setQueryMetadata(null);
    } finally {
      setIsLoading(false);
    }
  };

  const saveChanges = async () => {
    if (!queryMetadata?.tableName || !queryMetadata.primaryKeyColumn || changes.length === 0) {
      return;
    }

    setIsSaving(true);
    setError(null);

    try {
      // Generate and execute UPDATE statements
      for (const change of changes) {
        const updateSql = generateUpdateSql(
          queryMetadata.tableName,
          queryMetadata.primaryKeyColumn,
          change.primaryKeyValue,
          change.changes
        );

        await api.runQuery(projectId, updateSql, false);
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
          {queryMetadata?.isEditable && (
            <span className="ml-2 text-green-500">• Editable</span>
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
              columnLabels={columns}
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
