import { Play, Plus, RefreshCw, Save, Table, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
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

interface Tab {
  id: string;
  name: string;
  sql: string;
  results: SpreadsheetData;
  originalResults: SpreadsheetData;
  displayColumns: string[];
  queryMetadata: QueryMetadata | null;
  error: string | null;
}

function generateTabId(): string {
  return `tab-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

function createNewTab(): Tab {
  return {
    id: generateTabId(),
    name: "Untitled",
    sql: "SELECT * FROM ",
    results: [],
    originalResults: [],
    displayColumns: [],
    queryMetadata: null,
    error: null,
  };
}

// Extract the primary table name from a SQL query
function extractPrimaryTableName(sql: string): string | null {
  const normalized = sql.replace(/\s+/g, " ").trim();
  const fromMatch = normalized.match(/\bfrom\s+([a-z_][a-z0-9_]*)/i);
  return fromMatch ? fromMatch[1] : null;
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
  const [tabs, setTabs] = useState<Tab[]>(() => [createNewTab()]);
  const [activeTabId, setActiveTabId] = useState<string>(() => tabs[0]?.id || "");
  const [editingTabId, setEditingTabId] = useState<string | null>(null);
  const [editingTabName, setEditingTabName] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isLoadingTables, setIsLoadingTables] = useState(false);
  const editInputRef = useRef<HTMLInputElement>(null);

  // Fetch database tables using introspection query
  const fetchTables = useCallback(async () => {
    setIsLoadingTables(true);
    try {
      const result = await api.runQuery(
        projectId,
        `SELECT table_name FROM information_schema.tables
         WHERE table_schema = 'public'
         AND table_type = 'BASE TABLE'
         ORDER BY table_name`,
        true
      );
      if (Array.isArray(result)) {
        const tableNames = result.map((row: { table_name: string }) => row.table_name);

        // Create tabs for each table (preserving any existing custom tabs)
        const existingTabNames = new Set(tabs.map(t => t.name));
        const newTabs: Tab[] = tableNames
          .filter(name => !existingTabNames.has(name))
          .map(tableName => ({
            id: generateTabId(),
            name: tableName,
            sql: `SELECT * FROM "${tableName}" LIMIT 100`,
            results: [],
            originalResults: [],
            displayColumns: [],
            queryMetadata: null,
            error: null,
          }));

        if (newTabs.length > 0) {
          setTabs(prev => {
            // Remove the default "Untitled" tab if it exists and is empty
            const filtered = prev.filter(t => !(t.name === "Untitled" && t.sql === "SELECT * FROM "));
            const combined = [...filtered, ...newTabs];
            // If we removed all tabs, use the new ones
            if (combined.length === 0) return newTabs;
            return combined;
          });
          // Set active tab to first table if currently on untitled
          if (tabs.length === 1 && tabs[0].name === "Untitled") {
            setActiveTabId(newTabs[0]?.id || tabs[0]?.id || "");
          }
        }
      }
    } catch (err) {
      console.error("Failed to fetch tables:", err);
    } finally {
      setIsLoadingTables(false);
    }
  }, [projectId, tabs]);

  // Fetch tables on mount
  useEffect(() => {
    fetchTables();
  }, [projectId]); // Only run when projectId changes, not on every fetchTables change

  // Get current tab
  const currentTab = tabs.find((t) => t.id === activeTabId) || tabs[0];

  // Derived state from current tab
  const sql = currentTab?.sql || "";
  const results = currentTab?.results || [];
  const originalResults = currentTab?.originalResults || [];
  const displayColumns = currentTab?.displayColumns || [];
  const queryMetadata = currentTab?.queryMetadata || null;
  const error = currentTab?.error || null;

  // Update current tab helper
  const updateCurrentTab = useCallback((updates: Partial<Tab>) => {
    setTabs((prevTabs) =>
      prevTabs.map((tab) =>
        tab.id === activeTabId ? { ...tab, ...updates } : tab
      )
    );
  }, [activeTabId]);

  // Set SQL for current tab
  const setSql = useCallback((newSql: string) => {
    updateCurrentTab({ sql: newSql });
  }, [updateCurrentTab]);

  // Focus edit input when editing starts
  useEffect(() => {
    if (editingTabId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingTabId]);

  // Tab management functions
  const addNewTab = useCallback(() => {
    const newTab = createNewTab();
    setTabs((prev) => [...prev, newTab]);
    setActiveTabId(newTab.id);
  }, []);

  const closeTab = useCallback((tabId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    setTabs((prevTabs) => {
      if (prevTabs.length === 1) {
        // Don't close the last tab, just reset it
        return [createNewTab()];
      }
      const newTabs = prevTabs.filter((t) => t.id !== tabId);
      // If we're closing the active tab, switch to another
      if (tabId === activeTabId) {
        const closedIndex = prevTabs.findIndex((t) => t.id === tabId);
        const newActiveIndex = Math.min(closedIndex, newTabs.length - 1);
        setActiveTabId(newTabs[newActiveIndex].id);
      }
      return newTabs;
    });
  }, [activeTabId]);

  const startEditingTab = useCallback((tabId: string) => {
    const tab = tabs.find((t) => t.id === tabId);
    if (tab) {
      setEditingTabId(tabId);
      setEditingTabName(tab.name);
    }
  }, [tabs]);

  const finishEditingTab = useCallback(() => {
    if (editingTabId && editingTabName.trim()) {
      setTabs((prevTabs) =>
        prevTabs.map((tab) =>
          tab.id === editingTabId
            ? { ...tab, name: editingTabName.trim() }
            : tab
        )
      );
    }
    setEditingTabId(null);
    setEditingTabName("");
  }, [editingTabId, editingTabName]);

  const handleTabKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      finishEditingTab();
    } else if (e.key === "Escape") {
      setEditingTabId(null);
      setEditingTabName("");
    }
  }, [finishEditingTab]);

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
    updateCurrentTab({ error: null });

    try {
      const result = await api.runQuery(projectId, sql, true);

      if (Array.isArray(result) && result.length > 0) {
        const resultCols = Object.keys(result[0]);

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

        // Auto-rename untitled tabs to the primary table name
        const tabUpdates: Partial<Tab> = {
          displayColumns: resultCols,
          queryMetadata: metadata,
          results: data,
          originalResults: JSON.parse(JSON.stringify(data)),
          error: null,
        };

        if (currentTab?.name === "Untitled") {
          const primaryTable = extractPrimaryTableName(sql);
          if (primaryTable) {
            tabUpdates.name = primaryTable;
          }
        }

        updateCurrentTab(tabUpdates);
      } else {
        updateCurrentTab({
          displayColumns: [],
          results: [],
          originalResults: [],
          queryMetadata: null,
        });
      }
    } catch (err) {
      console.error("Query failed:", err);
      updateCurrentTab({
        error: typeof err === "string" ? err : String(err),
        results: [],
        originalResults: [],
        displayColumns: [],
        queryMetadata: null,
      });
    } finally {
      setIsLoading(false);
    }
  };

  const saveChanges = async () => {
    if (!queryMetadata?.isEditable || changes.length === 0) return;

    setIsSaving(true);
    updateCurrentTab({ error: null });

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
      updateCurrentTab({ originalResults: JSON.parse(JSON.stringify(results)) });
    } catch (err) {
      console.error("Save failed:", err);
      updateCurrentTab({ error: typeof err === "string" ? err : String(err) });
    } finally {
      setIsSaving(false);
    }
  };

  const handleDataChange = useCallback((newData: SpreadsheetData) => {
    updateCurrentTab({ results: newData });
  }, [updateCurrentTab]);

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
    <div className="flex h-full overflow-hidden">
      {/* Vertical Tabs Sidebar */}
      <div className="w-48 shrink-0 flex flex-col border-r bg-muted/20">
        {/* Sidebar Header */}
        <div className="shrink-0 flex items-center justify-between px-3 py-2 border-b">
          <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Tables</span>
          <div className="flex items-center gap-1">
            <button
              onClick={fetchTables}
              disabled={isLoadingTables}
              className="p-1 hover:bg-muted rounded transition-colors"
              title="Refresh tables"
            >
              <RefreshCw size={14} className={`text-muted-foreground ${isLoadingTables ? "animate-spin" : ""}`} />
            </button>
            <button
              onClick={addNewTab}
              className="p-1 hover:bg-muted rounded transition-colors"
              title="New query tab"
            >
              <Plus size={14} className="text-muted-foreground" />
            </button>
          </div>
        </div>

        {/* Tabs List */}
        <div className="flex-1 overflow-y-auto py-1">
          {tabs.map((tab) => (
            <div
              key={tab.id}
              onClick={() => setActiveTabId(tab.id)}
              onDoubleClick={() => startEditingTab(tab.id)}
              className={`group flex items-center gap-2 px-3 py-1.5 cursor-pointer transition-colors ${
                tab.id === activeTabId
                  ? "bg-primary/10 text-primary border-l-2 border-l-primary"
                  : "hover:bg-muted/50 border-l-2 border-l-transparent"
              }`}
            >
              <Table size={14} className="shrink-0 text-muted-foreground" />
              {editingTabId === tab.id ? (
                <input
                  ref={editInputRef}
                  type="text"
                  value={editingTabName}
                  onChange={(e) => setEditingTabName(e.target.value)}
                  onBlur={finishEditingTab}
                  onKeyDown={handleTabKeyDown}
                  className="flex-1 bg-transparent border-none outline-none text-sm min-w-0"
                  onClick={(e) => e.stopPropagation()}
                />
              ) : (
                <span className="flex-1 text-sm truncate" title={tab.name}>
                  {tab.name}
                </span>
              )}
              <button
                onClick={(e) => closeTab(tab.id, e)}
                className="shrink-0 opacity-0 group-hover:opacity-100 hover:bg-muted rounded p-0.5 transition-opacity"
                title="Close tab"
              >
                <X size={12} className="text-muted-foreground" />
              </button>
            </div>
          ))}
          {tabs.length === 0 && (
            <div className="px-3 py-4 text-center text-muted-foreground text-xs">
              No tables found
            </div>
          )}
        </div>
      </div>

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col overflow-hidden">
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
    </div>
  );
}
