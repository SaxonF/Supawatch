import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as api from "../api";
import { SqlChangesBar } from "./sql-editor/SqlChangesBar";
import { SqlQueryArea } from "./sql-editor/SqlQueryArea";
import { SqlResultsArea } from "./sql-editor/SqlResultsArea";
import { SqlSidebar } from "./sql-editor/SqlSidebar";
import {
  CellData,
  QueryMetadata,
  RowChanges,
  SpreadsheetData,
  SqlEditorProps,
  Tab,
  TableChange,
  TableRef,
} from "./sql-editor/types";
import {
  createNewTab,
  extractPrimaryTableName,
  findPrimaryKeys,
  formatCellValue,
  generateTabId,
  generateUpdateSql,
  hasNonEditableConstructs,
  parseColumns,
  parseTables,
  TABLES_QUERY,
} from "./sql-editor/utils";

export function SqlEditor({ projectId }: SqlEditorProps) {
  const [tabs, setTabs] = useState<Tab[]>(() => [createNewTab()]);
  const [activeTabId, setActiveTabId] = useState<string>(
    () => tabs[0]?.id || ""
  );
  const [editingTabId, setEditingTabId] = useState<string | null>(null);
  const [editingTabName, setEditingTabName] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isLoadingTables, setIsLoadingTables] = useState(false);
  const [tablesCollapsed, setTablesCollapsed] = useState(false);
  const editInputRef = useRef<HTMLInputElement>(null);

  // Fetch database tables using introspection query (matches backend logic)
  const fetchTables = useCallback(async () => {
    setIsLoadingTables(true);
    try {
      const result = await api.runQuery(projectId, TABLES_QUERY, true);
      if (Array.isArray(result)) {
        const tableRefs: TableRef[] = result.map(
          (row: { schema: string; name: string }) => ({
            schema: row.schema,
            name: row.name,
          })
        );

        // Create display name for tab - use schema prefix for non-public schemas
        const getDisplayName = (t: TableRef) =>
          t.schema === "public" ? t.name : `${t.schema}.${t.name}`;

        // Create SQL query - always use schema-qualified name for clarity
        const getSqlQuery = (t: TableRef) =>
          t.schema === "public"
            ? `SELECT * FROM ${t.name} LIMIT 100`
            : `SELECT * FROM ${t.schema}.${t.name} LIMIT 100`;

        // Only add tabs for tables that don't already exist
        // Perform duplicate check inside setTabs to use the current state
        setTabs((prev) => {
          const existingTabNames = new Set(prev.map((t) => t.name));
          const newTabs: Tab[] = tableRefs
            .filter((t) => !existingTabNames.has(getDisplayName(t)))
            .map((tableRef) => ({
              id: generateTabId(),
              name: getDisplayName(tableRef),
              sql: getSqlQuery(tableRef),
              results: [],
              originalResults: [],
              displayColumns: [],
              queryMetadata: null,
              error: null,
              isTableTab: true,
            }));

          if (newTabs.length === 0) return prev;
          return [...prev, ...newTabs];
        });
      }
    } catch (err) {
      console.error("Failed to fetch tables:", err);
    } finally {
      setIsLoadingTables(false);
    }
  }, [projectId]);

  // Fetch tables on mount
  useEffect(() => {
    fetchTables();
  }, [projectId]); // Only run when projectId changes, not on every fetchTables change

  // Get current tab
  const currentTab = tabs.find((t) => t.id === activeTabId) || tabs[0];

  // Split tabs into table tabs and other tabs
  const tableTabs = useMemo(() => tabs.filter((t) => t.isTableTab), [tabs]);
  const otherTabs = useMemo(() => tabs.filter((t) => !t.isTableTab), [tabs]);

  // Derived state from current tab
  const sql = currentTab?.sql || "";
  const results = currentTab?.results || [];
  const originalResults = currentTab?.originalResults || [];
  const displayColumns = currentTab?.displayColumns || [];
  const queryMetadata = currentTab?.queryMetadata || null;
  const error = currentTab?.error || null;

  // Update current tab helper
  const updateCurrentTab = useCallback(
    (updates: Partial<Tab>) => {
      setTabs((prevTabs) =>
        prevTabs.map((tab) =>
          tab.id === activeTabId ? { ...tab, ...updates } : tab
        )
      );
    },
    [activeTabId]
  );

  // Set SQL for current tab
  const setSql = useCallback(
    (newSql: string) => {
      updateCurrentTab({ sql: newSql });
    },
    [updateCurrentTab]
  );

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

  const closeTab = useCallback(
    (tabId: string, e: React.MouseEvent) => {
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
    },
    [activeTabId]
  );

  const startEditingTab = useCallback(
    (tabId: string) => {
      const tab = tabs.find((t) => t.id === tabId);
      if (tab) {
        setEditingTabId(tabId);
        setEditingTabName(tab.name);
      }
    },
    [tabs]
  );

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

  const handleTabKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        finishEditingTab();
      } else if (e.key === "Escape") {
        setEditingTabId(null);
        setEditingTabName("");
      }
    },
    [finishEditingTab]
  );

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
        if (colInfo.isComputed || colInfo.isPrimaryKey || !colInfo.tableName)
          continue;

        const currentValue = currentCell?.value ?? "";
        const originalValue = originalCell?.value ?? "";

        if (currentValue !== originalValue) {
          // Find the table info
          const tableInfo = queryMetadata.tables.find(
            (t) => t.name === colInfo.tableName
          );
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

    return {
      totalChanges,
      rowCount: changes.length,
      tableCount: tablesAffected.size,
    };
  }, [changes]);

  const runQuery = useCallback(async () => {
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
  }, [sql, projectId, updateCurrentTab, currentTab?.name]);

  // Auto-run query when a table tab is activated and hasn't been run yet
  useEffect(() => {
    if (
      currentTab?.isTableTab &&
      !currentTab.queryMetadata &&
      !isLoading &&
      !error
    ) {
      runQuery();
    }
  }, [
    activeTabId,
    currentTab?.isTableTab,
    currentTab?.queryMetadata,
    isLoading,
    error,
    runQuery,
  ]);

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
      updateCurrentTab({
        originalResults: JSON.parse(JSON.stringify(results)),
      });
    } catch (err) {
      console.error("Save failed:", err);
      updateCurrentTab({ error: typeof err === "string" ? err : String(err) });
    } finally {
      setIsSaving(false);
    }
  };

  const handleDataChange = useCallback(
    (newData: SpreadsheetData) => {
      updateCurrentTab({ results: newData });
    },
    [updateCurrentTab]
  );

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      runQuery();
    }
  };

  const discardChanges = useCallback(() => {
    updateCurrentTab({
      results: JSON.parse(JSON.stringify(originalResults)),
    });
  }, [updateCurrentTab, originalResults]);

  const hasChanges = changesSummary.totalChanges > 0;

  return (
    <div className="flex h-full overflow-hidden">
      <SqlSidebar
        tableTabs={tableTabs}
        otherTabs={otherTabs}
        activeTabId={activeTabId}
        setActiveTabId={setActiveTabId}
        startEditingTab={startEditingTab}
        editingTabId={editingTabId}
        editInputRef={editInputRef}
        editingTabName={editingTabName}
        setEditingTabName={setEditingTabName}
        finishEditingTab={finishEditingTab}
        handleTabKeyDown={handleTabKeyDown}
        closeTab={closeTab}
        tablesCollapsed={tablesCollapsed}
        setTablesCollapsed={setTablesCollapsed}
        fetchTables={fetchTables}
        isLoadingTables={isLoadingTables}
        addNewTab={addNewTab}
      />

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col overflow-hidden gap-0">
        <SqlQueryArea
          sql={sql}
          setSql={setSql}
          runQuery={runQuery}
          isLoading={isLoading}
          handleKeyDown={handleKeyDown}
        />

        <SqlResultsArea
          error={error}
          results={results}
          displayColumns={displayColumns}
          handleDataChange={handleDataChange}
        />

        {hasChanges && (
          <SqlChangesBar
            totalChanges={changesSummary.totalChanges}
            rowCount={changesSummary.rowCount}
            tableCount={changesSummary.tableCount}
            saveChanges={saveChanges}
            discardChanges={discardChanges}
            isSaving={isSaving}
          />
        )}
      </div>
    </div>
  );
}
