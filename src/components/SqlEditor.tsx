import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as api from "../api";
import * as store from "../utils/store";
import { SpecSidebar } from "./sql-editor/SpecSidebar";
import { SqlChangesBar } from "./sql-editor/SqlChangesBar";
import { SqlFormArea } from "./sql-editor/SqlFormArea";
import { SqlQueryArea } from "./sql-editor/SqlQueryArea";
import { SqlResultsArea } from "./sql-editor/SqlResultsArea";
import {
  CellData,
  QueryMetadata,
  RowChanges,
  SpreadsheetData,
  SqlEditorProps,
  Tab,
  TableChange,
} from "./sql-editor/types";
import {
  createNewTab,
  createSpecTab,
  extractPrimaryTableName,
  findPrimaryKeys,
  formatCellValue,
  generateUpdateSql,
  hasNonEditableConstructs,
  interpolateTemplate,
  parseColumns,
  parseTables,
  resolveActiveItem,
} from "./sql-editor/utils";
import { Button } from "./ui/button";

export function SqlEditor({ projectId }: SqlEditorProps) {
  const [tabs, setTabs] = useState<Tab[]>([createNewTab()]);

  const [activeTabId, setActiveTabId] = useState<string>("");
  const [editingTabId, setEditingTabId] = useState<string | null>(null);
  const [editingTabName, setEditingTabName] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isProcessingWithAI, setIsProcessingWithAI] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const editInputRef = useRef<HTMLInputElement>(null);

  // Load state and fetch tables when project changes
  useEffect(() => {
    // Reset state for new project immediately
    setEditingTabId(null);
    setEditingTabName("");
    setIsLoading(true);

    const loadState = async () => {
      try {
        const persistedTabs = await store.load<Tab[]>(
          store.PROJECT_KEYS.tabs(projectId),
        );
        const persistedActiveTab = await store.load<string>(
          store.PROJECT_KEYS.activeTab(projectId),
        );

        // Sanitize loaded tabs
        const safeTabs =
          persistedTabs && Array.isArray(persistedTabs)
            ? persistedTabs.map((t) => ({
                ...t,
                results: [],
                originalResults: [],
                queryMetadata: null,
                error: null,
              }))
            : null;

        if (safeTabs && safeTabs.length > 0) {
          setTabs(safeTabs);
          if (
            persistedActiveTab &&
            safeTabs.some((t) => t.id === persistedActiveTab)
          ) {
            setActiveTabId(persistedActiveTab);
          } else {
            setActiveTabId(safeTabs[0]?.id || "");
          }
        } else {
          // Reset to a fresh state with a new default tab
          const newTab = createNewTab();
          setTabs([newTab]);
          setActiveTabId(newTab.id);
        }
      } catch (err) {
        console.error("Failed to load state", err);
        // Fallback to default
        const newTab = createNewTab();
        setTabs([newTab]);
        setActiveTabId(newTab.id);
      } finally {
        setIsLoading(false);
      }
    };

    loadState();
  }, [projectId]); // Only run when projectId changes

  // Persist tabs when changed
  useEffect(() => {
    if (tabs.length > 0) {
      // Sanitize before saving
      const tabsToSave = tabs.map((t) => ({
        ...t,
        results: [],
        originalResults: [],
        queryMetadata: null,
        error: null,
      }));
      store.save(store.PROJECT_KEYS.tabs(projectId), tabsToSave);
    }
  }, [projectId, tabs]);

  // Persist active tab when changed
  useEffect(() => {
    if (activeTabId) {
      store.save(store.PROJECT_KEYS.activeTab(projectId), activeTabId);
    }
  }, [projectId, activeTabId]);

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
  const updateCurrentTab = useCallback(
    (updates: Partial<Tab>) => {
      setTabs((prevTabs) =>
        prevTabs.map((tab) =>
          tab.id === activeTabId ? { ...tab, ...updates } : tab,
        ),
      );
    },
    [activeTabId],
  );

  // Set SQL for current tab
  const setSql = useCallback(
    (newSql: string) => {
      updateCurrentTab({ sql: newSql });
    },
    [updateCurrentTab],
  );

  // Focus edit input when editing starts
  useEffect(() => {
    if (editingTabId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingTabId]);

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
    [activeTabId],
  );

  const startEditingTab = useCallback(
    (tabId: string) => {
      const tab = tabs.find((t) => t.id === tabId);
      if (tab) {
        setEditingTabId(tabId);
        setEditingTabName(tab.name);
      }
    },
    [tabs],
  );

  const finishEditingTab = useCallback(() => {
    if (editingTabId && editingTabName.trim()) {
      setTabs((prevTabs) =>
        prevTabs.map((tab) =>
          tab.id === editingTabId
            ? { ...tab, name: editingTabName.trim() }
            : tab,
        ),
      );
    }
    setEditingTabId(null);
    setEditingTabName("");
  }, [editingTabId, editingTabName]);

  const handleTabKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLInputElement>) => {
      if (e.key === "Enter") {
        e.preventDefault();
        finishEditingTab();
      } else if (e.key === "Escape") {
        setEditingTabId(null);
        setEditingTabName("");
      }
    },
    [finishEditingTab],
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
            (t) => t.name === colInfo.tableName,
          );
          if (!tableInfo || !tableInfo.primaryKeyColumn) continue;

          // Get primary key value for this table
          const pkColIdx = queryMetadata.columns.findIndex(
            (c) => c.resultName === tableInfo.primaryKeyColumn,
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

  const runQuery = useCallback(
    async (queryOverride?: unknown) => {
      // Handle potential event object from onClick
      const isOverride = typeof queryOverride === "string";
      const actualSql = isOverride ? queryOverride : sql;

      if (!actualSql.trim()) return;

      setIsLoading(true);
      updateCurrentTab({ error: null });

      let queryToRun = actualSql;
      let timeoutId: NodeJS.Timeout | null = null;

      try {
        // First, validate the SQL syntax
        try {
          await api.validateSql(actualSql);
        } catch (validationError) {
          // SQL is invalid - try to convert with AI
          console.log(
            "SQL validation failed, trying AI conversion:",
            validationError,
          );
          setIsProcessingWithAI(true);
          setIsLoading(false); // Stop regular loading, show AI indicator instead

          try {
            // Use full schema introspection for AI context
            const convertedSql = await api.convertWithAi(projectId, actualSql);

            // Update the SQL in the editor with the converted version
            queryToRun = convertedSql;
            updateCurrentTab({ sql: convertedSql });
            setIsProcessingWithAI(false);
            setIsLoading(true); // Resume loading for query execution
          } catch (aiError) {
            // AI conversion failed - show original validation error
            const errorMessage =
              validationError instanceof Error
                ? validationError.message
                : String(validationError);
            const aiErrorMessage =
              aiError instanceof Error ? aiError.message : String(aiError);

            updateCurrentTab({
              error: `Invalid SQL: ${errorMessage}. AI conversion failed: ${aiErrorMessage}`,
              results: [],
              originalResults: [],
              displayColumns: [],
              queryMetadata: null,
            });
            setIsProcessingWithAI(false);
            return;
          }
        }

        // Run the (possibly converted) query with a timeout
        const queryPromise = api.runQuery(projectId, queryToRun, false);
        const timeoutPromise = new Promise((_, reject) => {
          timeoutId = setTimeout(() => {
            reject(new Error("Query timed out after 10 seconds"));
          }, 10000); // 30 second timeout
        });

        const result = (await Promise.race([
          queryPromise,
          timeoutPromise,
        ])) as any;

        if (Array.isArray(result) && result.length > 0) {
          const resultCols = Object.keys(result[0]);

          // Parse query structure
          const tables = parseTables(queryToRun);
          const hasNonEditable = hasNonEditableConstructs(queryToRun);
          const columns = parseColumns(queryToRun, resultCols, tables);

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
          const data: CellData[][] = result.map(
            (row: Record<string, unknown>) =>
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
              }),
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
            const primaryTable = extractPrimaryTableName(queryToRun);
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
      } catch (err: unknown) {
        console.error("Query failed:", err);
        const errorMessage =
          err instanceof Error
            ? err.message
            : typeof err === "string"
              ? err
              : JSON.stringify(err);

        updateCurrentTab({
          error: errorMessage,
          results: [],
          originalResults: [],
          displayColumns: [],
          queryMetadata: null,
        });
      } finally {
        if (timeoutId) clearTimeout(timeoutId);
        setIsLoading(false);
        setIsProcessingWithAI(false);
      }
    },
    [sql, projectId, updateCurrentTab, currentTab?.name],
  );

  // Auto-run query when a table tab is activated and hasn't been run yet
  // Auto-run query when a table tab is activated and hasn't been run yet
  useEffect(() => {
    // Resolve active item for spec tabs to check its specific autoRun property
    const activeSpecItem = currentTab?.specItem
      ? resolveActiveItem(currentTab.specItem, currentTab.viewStack).item
      : null;

    if (
      // Auto-run if active spec item says so (not just the root item), or legacy table tab check
      (activeSpecItem?.autoRun || currentTab?.isTableTab) &&
      !currentTab.queryMetadata &&
      !isLoading &&
      !error
    ) {
      runQuery();
    }
  }, [
    activeTabId,
    currentTab?.specItem,
    currentTab?.viewStack,
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
            tableChange.changes,
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
    [updateCurrentTab],
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

  const handleFixQuery = useCallback(async () => {
    if (!sql.trim()) return;
    setIsProcessingWithAI(true);
    try {
      const convertedSql = await api.convertWithAi(projectId, sql);
      updateCurrentTab({ sql: convertedSql, error: null });
      await runQuery(convertedSql);
    } catch (err) {
      updateCurrentTab({ error: `AI Fix failed: ${err}` });
    } finally {
      setIsProcessingWithAI(false);
    }
  }, [projectId, sql, updateCurrentTab, runQuery]);

  return (
    <div className="flex h-full overflow-hidden">
      <SpecSidebar
        projectId={projectId}
        tabs={tabs}
        activeTabId={activeTabId}
        onTabSelect={setActiveTabId}
        onTabCreate={(groupId, item, params) => {
          const newTab = createSpecTab(groupId, item, params || {});
          setTabs((prev) => [...prev, newTab]);
          setActiveTabId(newTab.id);
        }}
        onTabClose={closeTab}
        onTabRename={(tabId, name) => {
          setTabs((prev) =>
            prev.map((t) => (t.id === tabId ? { ...t, name } : t)),
          );
        }}
        startEditingTab={startEditingTab}
        editingTabId={editingTabId}
        editInputRef={editInputRef}
        editingTabName={editingTabName}
        setEditingTabName={setEditingTabName}
        finishEditingTab={finishEditingTab}
        handleTabKeyDown={handleTabKeyDown}
      />

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col overflow-hidden gap-0">
        {/* Resolve active spec item for navigation */}
        {(() => {
          // Helper to resolve active item - default to current tab state if no spec logic
          const activeSpecItem = currentTab.specItem
            ? resolveActiveItem(currentTab.specItem, currentTab.viewStack).item
            : null;

          const activeParams = currentTab.specItem
            ? resolveActiveItem(currentTab.specItem, currentTab.viewStack)
                .params
            : {};

          return (
            <>
              {/* Header for Spec Tabs */}
              {activeSpecItem && (
                <div className="border-b px-4 py-2 flex items-center justify-between bg-muted/20 shrink-0 min-h-[42px]">
                  <div className="flex items-center gap-2">
                    {currentTab.viewStack && currentTab.viewStack.length > 1 ? (
                      <>
                        <button
                          onClick={() => {
                            setTabs((prev) =>
                              prev.map((t) => {
                                if (t.id !== activeTabId) return t;
                                const newStack =
                                  t.viewStack?.slice(0, -1) || [];
                                const { item: prevItem, params: prevParams } =
                                  resolveActiveItem(t.specItem!, newStack);
                                return {
                                  ...t,
                                  viewStack: newStack,
                                  sql: prevItem.sql
                                    ? interpolateTemplate(
                                        prevItem.sql,
                                        prevParams,
                                      )
                                    : "",
                                  formValues: {},
                                  results: [],
                                  queryMetadata: null, // Clear metadata to trigger auto-run if applicable
                                  error: null,
                                };
                              }),
                            );
                          }}
                          className="text-sm font-medium text-muted-foreground flex items-center gap-1 font-medium"
                        >
                          Back
                        </button>
                        <div className="text-muted-foreground/25">/</div>
                      </>
                    ) : null}
                    <span className="text-sm font-medium">
                      {activeSpecItem.name}
                    </span>
                  </div>

                  {/* Primary Action */}
                  {activeSpecItem.primaryAction && (
                    <div className="flex items-center">
                      <Button
                        size="sm"
                        onClick={() => {
                          const action = activeSpecItem.primaryAction!;
                          setTabs((prev) =>
                            prev.map((t) => {
                              if (t.id !== activeTabId) return t;
                              const currentStack = t.viewStack || [];
                              const newParams = { ...activeParams };
                              const newStack = [
                                ...currentStack,
                                { itemId: action.itemId, params: newParams },
                              ];
                              const { item: newItem, params: finalParams } =
                                resolveActiveItem(t.specItem!, newStack);

                              return {
                                ...t,
                                viewStack: newStack,
                                sql: newItem.sql
                                  ? interpolateTemplate(
                                      newItem.sql,
                                      finalParams,
                                    )
                                  : "",
                                formValues: {},
                                results: [],
                                error: null,
                              };
                            }),
                          );
                        }}
                      >
                        {activeSpecItem.primaryAction.label}
                      </Button>
                    </div>
                  )}
                </div>
              )}

              <SqlQueryArea
                sql={sql}
                setSql={setSql}
                runQuery={runQuery}
                isLoading={isLoading}
                isProcessingWithAI={isProcessingWithAI}
                handleKeyDown={handleKeyDown}
              />

              {activeSpecItem?.type === "mutation" && activeSpecItem.form ? (
                <SqlFormArea
                  projectId={projectId}
                  sql={sql}
                  formConfig={activeSpecItem.form}
                  loadQuery={activeSpecItem.loadQuery}
                  params={activeParams}
                  formValues={currentTab.formValues || {}}
                  onFormValuesChange={(values) =>
                    setTabs((prev) =>
                      prev.map((t) => {
                        if (t.id !== activeTabId) return t;

                        // Helper to properly quote values for SQL interpolation
                        const quoteValue = (val: unknown) => {
                          if (val === null || val === undefined) return "NULL";
                          return `'${String(val).replace(/'/g, "''")}'`;
                        };

                        const params = {
                          ...Object.fromEntries(
                            Object.entries(activeParams).map(([k, v]) => [
                              k,
                              quoteValue(v),
                            ]),
                          ),
                          ...Object.fromEntries(
                            Object.entries(values).map(([k, v]) => [
                              k,
                              quoteValue(v),
                            ]),
                          ),
                        };

                        return {
                          ...t,
                          formValues: values,
                          sql: activeSpecItem.sql
                            ? interpolateTemplate(activeSpecItem.sql, params)
                            : t.sql,
                        };
                      }),
                    )
                  }
                  onSubmit={runQuery}
                  onCancel={
                    currentTab.viewStack && currentTab.viewStack.length > 1
                      ? () => {
                          setTabs((prev) =>
                            prev.map((t) => {
                              if (t.id !== activeTabId) return t;
                              const newStack = t.viewStack?.slice(0, -1) || [];
                              const { item: prevItem, params: prevParams } =
                                resolveActiveItem(t.specItem!, newStack);
                              return {
                                ...t,
                                viewStack: newStack,
                                sql: prevItem.sql
                                  ? interpolateTemplate(
                                      prevItem.sql,
                                      prevParams,
                                    )
                                  : "",
                                formValues: {},
                                results: [],
                                queryMetadata: null, // Clear metadata to trigger auto-run if applicable
                                error: null,
                              };
                            }),
                          );
                        }
                      : undefined
                  }
                  isSubmitting={isLoading}
                  error={error}
                />
              ) : (
                <SqlResultsArea
                  error={error}
                  results={results}
                  displayColumns={displayColumns}
                  handleDataChange={handleDataChange}
                  onFixQuery={handleFixQuery}
                  isProcessingWithAI={isProcessingWithAI}
                  rowActions={activeSpecItem?.rowActions}
                  onRowAction={(action, row) => {
                    setTabs((prev) =>
                      prev.map((t) => {
                        if (t.id !== activeTabId) return t;

                        const currentStack = t.viewStack || [];
                        const newParams: Record<string, string> = {
                          ...activeParams,
                        };

                        if (action.params) {
                          for (const [key, colName] of Object.entries(
                            action.params,
                          )) {
                            newParams[key] = String(row[colName] || "");
                          }
                        }

                        const newStack = [
                          ...currentStack,
                          { itemId: action.itemId, params: newParams },
                        ];
                        const { item: newItem, params: finalParams } =
                          resolveActiveItem(t.specItem!, newStack);

                        return {
                          ...t,
                          viewStack: newStack,
                          sql: newItem.sql
                            ? interpolateTemplate(newItem.sql, finalParams)
                            : "",
                          formValues: {},
                          results: [],
                          error: null,
                        };
                      }),
                    );
                  }}
                  chart={activeSpecItem?.chart}
                />
              )}
            </>
          );
        })()}

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
