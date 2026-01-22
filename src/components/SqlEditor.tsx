import { Plus } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import * as api from "../api";
import { defaultSidebarSpec } from "../specs";
import * as store from "../utils/store";
import { QueryBlock } from "./sql-editor/QueryBlock";
import { SpecSidebar } from "./sql-editor/SpecSidebar";
import { SqlChangesBar } from "./sql-editor/SqlChangesBar";
import {
  CellData,
  QueryMetadata,
  QueryState,
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
  generateQueryStates,
  generateUpdateSql,
  hasNonEditableConstructs,
  interpolateTemplate,
  parseColumns,
  parseTables,
  resolveActiveItem,
} from "./sql-editor/utils";
import { Button } from "./ui/button";

/**
 * Look up the original spec item from defaultSidebarSpec by groupId and itemId.
 * This ensures we always get the original SQL templates with proper quoting,
 * rather than using potentially corrupted persisted tab state.
 */
function getOriginalSpecItem(
  groupId: string,
  itemId: string,
): { id: string; queries?: any[]; children?: any[] } | null {
  const group = defaultSidebarSpec.groups.find((g) => g.id === groupId);
  if (!group) return null;

  // Check static items
  if (group.items) {
    const item = group.items.find((i) => i.id === itemId);
    if (item) return item;
  }

  // Check itemTemplate (for dynamic groups like tables or scripts)
  if (group.itemTemplate) {
    // For dynamic items, return the template BUT with the ID injected.
    // This is critical because validation logic often checks if specItem.id === currentItem.id.
    // Dynamic items like scripts have ":id" in the template, but the instance has a concrete ID.
    return {
      ...group.itemTemplate,
      id: itemId,
    };
  }

  return null;
}

export function SqlEditor({ projectId }: SqlEditorProps) {
  const [tabs, setTabs] = useState<Tab[]>([createNewTab()]);

  const [activeTabId, setActiveTabId] = useState<string>("");
  const [editingTabId, setEditingTabId] = useState<string | null>(null);
  const [editingTabName, setEditingTabName] = useState("");
  const [isLoading, setIsLoading] = useState(false);
  const [isProcessingWithAI, setIsProcessingWithAI] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const editInputRef = useRef<HTMLInputElement>(null);

  // Track if state has been initialized from store to prevent overwriting with default state
  const isInitialized = useRef(false);

  // Load state and fetch tables when project changes
  useEffect(() => {
    // Reset state for new project immediately
    setEditingTabId(null);
    setEditingTabName("");
    setIsLoading(true);

    const loadState = async () => {
      console.error(
        "[PERSISTENCE] loadState starting for projectId:",
        projectId,
      );
      try {
        const tabsKey = store.PROJECT_KEYS.tabs(projectId);
        console.error("[PERSISTENCE] Loading tabs with key:", tabsKey);

        const persistedTabs = await store.load<Tab[]>(tabsKey);

        if (!persistedTabs) {
          console.error(
            "[PERSISTENCE] Store returned null/undefined for key:",
            tabsKey,
          );
        }
        const persistedActiveTab = await store.load<string>(
          store.PROJECT_KEYS.activeTab(projectId),
        );

        // Sanitize loaded tabs - clear results and other large data
        const safeTabs =
          persistedTabs && Array.isArray(persistedTabs)
            ? persistedTabs.map((t) => {
                // Fix for missing groupId on scripts (persistence issue recovery)
                let groupId = t.groupId;
                // If it looks like a script (specItem.id matches tab.id) but has no groupId, assign it to scripts
                if (
                  !groupId &&
                  t.specItem &&
                  t.specItem.id === t.id &&
                  t.name
                ) {
                  groupId = "scripts";
                }

                return {
                  ...t,
                  groupId, // Ensure groupId is preserved/restored
                  results: [],
                  originalResults: [],
                  queryMetadata: null,
                  error: null,
                  // Sanitize queryStates as well
                  queryStates: t.queryStates?.map((qs: any) => ({
                    ...qs,
                    results: [],
                    originalResults: [],
                    queryMetadata: null,
                    error: null,
                  })),
                };
              })
            : null;

        if (safeTabs) {
          console.error(
            "[PERSISTENCE] SqlEditor loaded safeTabs:",
            safeTabs.length,
            safeTabs,
          );
        }

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
          console.error(
            "[PERSISTENCE] No persisted tabs found for project:",
            projectId,
            ". Resetting to default.",
          );
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
        isInitialized.current = true;
      }
    };

    loadState();
  }, [projectId]); // Only run when projectId changes

  // Persist tabs when changed
  useEffect(() => {
    // Prevent saving if we haven't loaded state yet (avoids overwriting store with default state on mount)
    if (!isInitialized.current) return;

    if (tabs.length > 0) {
      // Sanitize before saving - remove large data from tabs and queryStates
      const tabsToSave = tabs.map((t) => ({
        ...t,
        groupId: t.groupId, // Explicitly include groupId to ensure persistence
        results: [],
        originalResults: [],
        queryMetadata: null,
        error: null,
        // Sanitize queryStates as well
        queryStates: t.queryStates?.map((qs) => ({
          ...qs,
          results: [],
          originalResults: [],
          queryMetadata: null,
          error: null,
        })),
      }));

      const tabsKey = store.PROJECT_KEYS.tabs(projectId);
      if (tabsKey.includes("tabs")) {
        console.error(
          "[PERSISTENCE] Saving tabs:",
          JSON.stringify(tabsToSave, null, 2),
        );
      }
      store.save(tabsKey, tabsToSave);
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
  // Helper to calculate changes for a single query's results
  const calculateChangesForQuery = (
    qResults: SpreadsheetData,
    qOriginalResults: SpreadsheetData,
    qMetadata: QueryMetadata | null,
  ): RowChanges[] => {
    if (!qMetadata?.isEditable) return [];

    const rowChanges: RowChanges[] = [];

    for (let rowIdx = 0; rowIdx < qResults.length; rowIdx++) {
      const currentRow = qResults[rowIdx];
      const originalRow = qOriginalResults[rowIdx];
      if (!currentRow || !originalRow) continue;

      // Group changes by table
      const tableChangesMap: Record<string, TableChange> = {};

      for (let colIdx = 0; colIdx < qMetadata.columns.length; colIdx++) {
        const colInfo = qMetadata.columns[colIdx];
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
          const tableInfo = qMetadata.tables.find(
            (t) => t.name === colInfo.tableName,
          );
          if (!tableInfo || !tableInfo.primaryKeyColumn) continue;

          // Get primary key value for this table
          const pkColIdx = qMetadata.columns.findIndex(
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
  };

  const changes = useMemo((): RowChanges[] => {
    // If using queryStates, aggregate changes from all queries
    if (currentTab?.queryStates && currentTab.queryStates.length > 0) {
      const allChanges: RowChanges[] = [];
      for (const qs of currentTab.queryStates) {
        const qsChanges = calculateChangesForQuery(
          qs.results,
          qs.originalResults,
          qs.queryMetadata,
        );
        allChanges.push(...qsChanges);
      }
      return allChanges;
    }

    // Legacy: use tab-level results/metadata
    return calculateChangesForQuery(results, originalResults, queryMetadata);
  }, [results, originalResults, queryMetadata, currentTab?.queryStates]);

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
    async (queryOverride?: unknown, queryIndex?: number) => {
      // Handle potential event object from onClick
      const isOverride = typeof queryOverride === "string";

      // Determine which SQL to run
      let currentSql = sql;
      if (queryIndex !== undefined && currentTab.queryStates?.[queryIndex]) {
        currentSql = currentTab.queryStates[queryIndex].sql;
      }

      const actualSql = isOverride ? (queryOverride as string) : currentSql;

      if (!actualSql.trim()) return false;

      // Helper to update state for specific query or main tab
      const updateState = (updates: Partial<Tab> | Partial<QueryState>) => {
        setTabs((prevTabs) =>
          prevTabs.map((tab) => {
            if (tab.id !== activeTabId) return tab;

            if (queryIndex !== undefined && tab.queryStates) {
              const newQueryStates = [...tab.queryStates];
              newQueryStates[queryIndex] = {
                ...newQueryStates[queryIndex],
                ...updates,
              } as any;
              return { ...tab, queryStates: newQueryStates };
            }

            return { ...tab, ...updates };
          }),
        );
      };

      setIsLoading(true);
      updateState({ error: null });

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
            updateState({ sql: convertedSql });
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

            updateState({
              error: `Invalid SQL: ${errorMessage}. AI conversion failed: ${aiErrorMessage}`,
              results: [],
              originalResults: [],
              displayColumns: [],
              queryMetadata: null,
            });
            setIsProcessingWithAI(false);
            return false;
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
          const tabUpdates: any = {
            displayColumns: resultCols,
            queryMetadata: metadata,
            results: data,
            originalResults: JSON.parse(JSON.stringify(data)),
            error: null,
          };

          if (currentTab?.name === "Untitled" && queryIndex === undefined) {
            // Only auto-rename on main query
            const primaryTable = extractPrimaryTableName(queryToRun);
            if (primaryTable) {
              tabUpdates.name = primaryTable;
            }
          }

          updateState(tabUpdates);
          return true;
        } else {
          updateState({
            displayColumns: [],
            results: [],
            originalResults: [],
            queryMetadata: null,
            error: null,
          });
          return true;
        }
      } catch (err: unknown) {
        console.error("Query failed:", err);
        const errorMessage =
          err instanceof Error
            ? err.message
            : typeof err === "string"
              ? err
              : JSON.stringify(err);

        updateState({
          error: errorMessage,
          results: [],
          originalResults: [],
          displayColumns: [],
          queryMetadata: null, // Clear metadata
        });
        return false;
      } finally {
        if (timeoutId) clearTimeout(timeoutId);
        setIsLoading(false);
        setIsProcessingWithAI(false);
      }
    },
    [sql, projectId, activeTabId, currentTab?.name, currentTab?.queryStates],
  );

  // Auto-run query when a table tab is activated and hasn't been run yet
  useEffect(() => {
    // Resolve active item for spec tabs to check its specific autoRun property
    const activeSpecItem = currentTab?.specItem
      ? resolveActiveItem(currentTab.specItem, currentTab.viewStack).item
      : null;

    const shouldAutoRun = activeSpecItem?.autoRun || currentTab?.isTableTab;

    if (shouldAutoRun && !isLoading && !error) {
      if (currentTab?.queryStates) {
        // Run all queries that haven't been run or need update
        // We run them sequentially
        currentTab.queryStates.forEach((qs, idx) => {
          // Check if results are empty - rudimentary check for "needs run"
          // Ideally we'd have a specific "hasRun" flag or similar
          if (qs.results.length === 0 && !qs.error) {
            runQuery(undefined, idx);
          }
        });
      } else {
        // Legacy single query
        // Avoid infinite loops by checking if we have results?
        // But existing logic relied on !error and !isLoading + dependency array trickery
        // The previous code had a specific check:
        // (activeSpecItem?.autoRun || currentTab?.isTableTab) && !isLoading && !error
        runQuery();
      }
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps -- runQuery is called, not read; including it causes infinite loops
  }, [
    activeTabId,
    currentTab?.specItem,
    currentTab?.viewStack,
    currentTab?.isTableTab,
    // We add queryStates length to detect when it's initialized
    currentTab?.queryStates?.length,
    // runQuery is stable
  ]);

  const saveChanges = async () => {
    if (changes.length === 0) return;

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
      setTabs((prevTabs) =>
        prevTabs.map((tab) => {
          if (tab.id !== activeTabId) return tab;

          // Update queryStates originalResults if present
          const updatedQueryStates = tab.queryStates?.map((qs) => ({
            ...qs,
            originalResults: JSON.parse(JSON.stringify(qs.results)),
          }));

          return {
            ...tab,
            // Update legacy tab-level originalResults
            originalResults: JSON.parse(JSON.stringify(tab.results)),
            // Update queryStates originalResults
            queryStates: updatedQueryStates,
          };
        }),
      );
    } catch (err) {
      console.error("Save failed:", err);
      updateCurrentTab({ error: typeof err === "string" ? err : String(err) });
    } finally {
      setIsSaving(false);
    }
  };

  const discardChanges = useCallback(() => {
    setTabs((prevTabs) =>
      prevTabs.map((tab) => {
        if (tab.id !== activeTabId) return tab;

        // Revert queryStates if present
        const revertedQueryStates = tab.queryStates?.map((qs) => ({
          ...qs,
          results: JSON.parse(JSON.stringify(qs.originalResults)),
        }));

        return {
          ...tab,
          // Revert legacy tab-level results
          results: JSON.parse(JSON.stringify(tab.originalResults)),
          // Revert queryStates results
          queryStates: revertedQueryStates,
        };
      }),
    );
  }, [activeTabId]);

  const hasChanges = changesSummary.totalChanges > 0;

  const handleFixQuery = useCallback(
    async (errorOverride?: string) => {
      if (!sql.trim()) return;
      setIsProcessingWithAI(true);
      try {
        const errorToUse =
          typeof errorOverride === "string" && errorOverride
            ? errorOverride
            : error;

        const convertedSql = await api.convertWithAi(
          projectId,
          sql,
          errorToUse || undefined,
        );
        updateCurrentTab({ sql: convertedSql, error: null });
        await runQuery(convertedSql);
      } catch (err) {
        updateCurrentTab({ error: `AI Fix failed: ${err}` });
      } finally {
        setIsProcessingWithAI(false);
      }
    },
    [projectId, sql, error, updateCurrentTab, runQuery],
  );

  const handleBack = useCallback(() => {
    setTabs((prev) =>
      prev.map((t) => {
        if (t.id !== activeTabId) return t;

        // Get the ORIGINAL spec item from defaultSidebarSpec
        const originalRootItem = t.groupId
          ? getOriginalSpecItem(t.groupId, t.specItem?.id || "")
          : null;
        const rootItem = originalRootItem || t.specItem;
        if (!rootItem) return t;

        const newStack = t.viewStack?.slice(0, -1) || [];
        const { item: prevItem, params: prevParams } = resolveActiveItem(
          rootItem,
          newStack,
        );

        // Generate fresh query states for the parent item
        const newQueryStates = generateQueryStates(
          prevItem.queries || [],
          prevParams,
        );

        return {
          ...t,
          viewStack: newStack,
          sql: "",
          formValues: {},
          results: [],
          queryMetadata: null,
          error: null,
          queryStates: newQueryStates,
        };
      }),
    );
  }, [activeTabId]);

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
                          onClick={handleBack}
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

                              // Get the ORIGINAL spec item from defaultSidebarSpec
                              const originalRootItem = t.groupId
                                ? getOriginalSpecItem(
                                    t.groupId,
                                    t.specItem?.id || "",
                                  )
                                : null;
                              const rootItem = originalRootItem || t.specItem;
                              if (!rootItem) return t;

                              const currentStack = t.viewStack || [];
                              const newParams = { ...activeParams };
                              const newStack = [
                                ...currentStack,
                                { itemId: action.itemId, params: newParams },
                              ];
                              const { item: newItem, params: finalParams } =
                                resolveActiveItem(rootItem, newStack);

                              // Generate fresh query states for the new item
                              const newQueryStates = generateQueryStates(
                                newItem.queries || [],
                                finalParams,
                              );

                              return {
                                ...t,
                                viewStack: newStack,
                                sql: "",
                                formValues: {},
                                results: [],
                                error: null,
                                queryStates: newQueryStates,
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

              {currentTab.queryStates && (
                <div className="flex-1 flex flex-col overflow-auto">
                  {currentTab.queryStates.map((qs, idx) => (
                    <QueryBlock
                      key={`${activeSpecItem?.id || "root"}-${idx}`}
                      index={idx}
                      queryState={qs}
                      projectId={projectId}
                      activeParams={activeParams}
                      isProcessingWithAI={isProcessingWithAI}
                      isLoading={isLoading}
                      formValues={currentTab.formValues || {}}
                      onFixQuery={(queryIndex, errorMsg) => {
                        // Fix query for specific index
                        const queryState = currentTab.queryStates?.[queryIndex];
                        if (!queryState?.sql.trim()) return;
                        setIsProcessingWithAI(true);
                        api
                          .convertWithAi(projectId, queryState.sql, errorMsg)
                          .then((convertedSql) => {
                            setTabs((prev) =>
                              prev.map((t) => {
                                if (t.id !== activeTabId || !t.queryStates)
                                  return t;
                                const newStates = [...t.queryStates];
                                newStates[queryIndex] = {
                                  ...newStates[queryIndex],
                                  sql: convertedSql,
                                  error: null,
                                };
                                return { ...t, queryStates: newStates };
                              }),
                            );
                            // Re-run the query with fixed SQL
                            runQuery(convertedSql, queryIndex);
                          })
                          .catch((err) => {
                            setTabs((prev) =>
                              prev.map((t) => {
                                if (t.id !== activeTabId || !t.queryStates)
                                  return t;
                                const newStates = [...t.queryStates];
                                newStates[queryIndex] = {
                                  ...newStates[queryIndex],
                                  error: `AI Fix failed: ${err}`,
                                };
                                return { ...t, queryStates: newStates };
                              }),
                            );
                          })
                          .finally(() => setIsProcessingWithAI(false));
                      }}
                      onFormValuesChange={(
                        newValues: Record<string, unknown>,
                      ) => {
                        // We update the tab's formValues.
                        // In a multi-query scenario, we might want to merge, but simple set is fine for now
                        // assuming one form at a time or non-colliding keys.
                        setTabs((prev) =>
                          prev.map((t) => {
                            if (t.id !== activeTabId) return t;

                            // Get the ORIGINAL spec item from defaultSidebarSpec (not persisted state)
                            // to ensure SQL templates have proper quoting like ':param'
                            const originalRootItem = t.groupId
                              ? getOriginalSpecItem(
                                  t.groupId,
                                  t.specItem?.id || "",
                                )
                              : null;

                            // Use original spec if available, fall back to persisted specItem
                            const rootItem = originalRootItem || t.specItem;
                            if (!rootItem) return t;

                            // Resolve the ACTIVE item from the viewStack
                            const { item: activeItem } = resolveActiveItem(
                              rootItem,
                              t.viewStack,
                            );
                            const mergedParams = {
                              ...activeParams,
                              ...newValues,
                            } as Record<string, string>;

                            // Re-interpolate all queries based on new form values
                            // Use the ACTIVE item's queries as template
                            const rawQueries = activeItem.queries || [];

                            // We only want to update SQL/loadQuery, preserving other state like results
                            const updatedStates = t.queryStates!.map(
                              (existingQs, qIdx) => {
                                const rawQ = rawQueries[qIdx];
                                if (!rawQ) return existingQs;

                                return {
                                  ...existingQs,
                                  sql: interpolateTemplate(
                                    rawQ.sql,
                                    mergedParams,
                                  ),
                                  loadQuery: rawQ.loadQuery
                                    ? interpolateTemplate(
                                        rawQ.loadQuery,
                                        mergedParams,
                                      )
                                    : undefined,
                                };
                              },
                            );

                            return {
                              ...t,
                              formValues: { ...t.formValues, ...newValues },
                              queryStates: updatedStates,
                            };
                          }),
                        );
                      }}
                      onRunQuery={async (index: number) => {
                        const success = await runQuery(undefined, index);

                        // Check if the query config requires returning to parent
                        const queryConfig = activeSpecItem?.queries?.[index];
                        const shouldReturn =
                          queryConfig?.returnToParent ||
                          activeSpecItem?.returnToParent;

                        if (
                          success &&
                          shouldReturn &&
                          currentTab.viewStack &&
                          currentTab.viewStack.length > 0
                        ) {
                          handleBack();
                        }
                      }}
                      onSqlChange={(index: number, newSql: string) => {
                        setTabs((prev) =>
                          prev.map((t) => {
                            if (t.id !== activeTabId || !t.queryStates)
                              return t;
                            const newStates = [...t.queryStates];
                            newStates[index] = {
                              ...newStates[index],
                              sql: newSql,
                            };
                            return { ...t, queryStates: newStates };
                          }),
                        );
                      }}
                      onResultsChange={(index: number, newData: any) => {
                        setTabs((prev) =>
                          prev.map((t) => {
                            if (t.id !== activeTabId || !t.queryStates)
                              return t;
                            const newStates = [...t.queryStates];
                            newStates[index] = {
                              ...newStates[index],
                              results: newData,
                            };
                            return { ...t, queryStates: newStates };
                          }),
                        );
                      }}
                      onRowAction={(action: any, row: any) => {
                        setTabs((prev) =>
                          prev.map((t) => {
                            if (t.id !== activeTabId) return t;

                            // Get the ORIGINAL spec item from defaultSidebarSpec
                            const originalRootItem = t.groupId
                              ? getOriginalSpecItem(
                                  t.groupId,
                                  t.specItem?.id || "",
                                )
                              : null;
                            const rootItem = originalRootItem || t.specItem;
                            if (!rootItem) return t;

                            const currentStack = t.viewStack || [];
                            const newParams: Record<string, string> = {
                              ...activeParams,
                            };

                            if (action.params) {
                              for (const [key, colName] of Object.entries(
                                action.params,
                              )) {
                                newParams[key] = String(
                                  row[colName as string] || "",
                                );
                              }
                            }

                            const newStack = [
                              ...currentStack,
                              { itemId: action.itemId, params: newParams },
                            ];
                            const { item: newItem, params: finalParams } =
                              resolveActiveItem(rootItem, newStack);

                            // Use generateQueryStates to create fresh states for the new item
                            const newQueryStates = generateQueryStates(
                              newItem.queries || [],
                              finalParams,
                            );

                            return {
                              ...t,
                              viewStack: newStack,
                              // Legacy top-level sql cleared
                              sql: "",
                              formValues: {},
                              results: [],
                              error: null,
                              queryStates: newQueryStates,
                            };
                          }),
                        );
                      }}
                    />
                  ))}
                </div>
              )}

              {/* Add Query Button for User Creatable Groups */}
              {(() => {
                const group = defaultSidebarSpec.groups.find(
                  (g) => g.id === currentTab.groupId,
                );
                return (
                  group?.itemsFromState &&
                  group.userCreatable && (
                    <Button
                      variant="ghost"
                      className="w-full rounded-none"
                      onClick={() => {
                        setTabs((prev) =>
                          prev.map((t) => {
                            if (t.id !== activeTabId || !t.queryStates)
                              return t;

                            const newQueryState: QueryState = {
                              sql: "",
                              results: [],
                              originalResults: [],
                              displayColumns: [],
                              queryMetadata: null,
                              error: null,
                              resultsConfig: "table",
                            };

                            return {
                              ...t,
                              queryStates: [...t.queryStates, newQueryState],
                            };
                          }),
                        );
                      }}
                    >
                      <Plus strokeWidth={1} size={16} />
                    </Button>
                  )
                );
              })()}
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
