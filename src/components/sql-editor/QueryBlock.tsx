import * as api from "@/api";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertCircleIcon, Loader2, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { Button } from "../ui/button";
import { QueryInput } from "./QueryInput";
import { SqlFormArea } from "./SqlFormArea";
import { SqlQueryArea } from "./SqlQueryArea";
import { SqlResultsArea } from "./SqlResultsArea";
import { QueryState } from "./types";
import { interpolateTemplate } from "./utils";

interface QueryBlockProps {
  queryState: QueryState;
  index: number;
  projectId: string;
  activeParams: Record<string, string>;
  isProcessingWithAI: boolean;
  isLoading: boolean;
  onRunQuery: (index: number) => void;
  onSqlChange: (index: number, newSql: string) => void;
  onResultsChange: (index: number, results: any) => void;
  onRowAction: (action: any, row: any) => void;
  onFixQuery?: (index: number, error?: string) => void;
  formValues: Record<string, unknown>;
  onFormValuesChange: (values: Record<string, unknown>) => void;
  canRemove?: boolean;
  onRemove?: () => void;
}

export function QueryBlock({
  queryState: qs,
  index,
  projectId,
  activeParams,
  isProcessingWithAI,
  isLoading,
  onRunQuery,
  onSqlChange,
  onResultsChange,
  onRowAction,
  onFixQuery,
  formValues,
  onFormValuesChange,
  canRemove,
  onRemove,
}: QueryBlockProps) {
  const [mode, setMode] = useState<"form" | "sql">(
    qs.parameters?.length ? "form" : "sql",
  );
  const [isLoadingData, setIsLoadingData] = useState(false);

  // Load existing data if loader is defined (for edit forms)
  useEffect(() => {
    if (!qs.loader || !qs.parameters?.length) return;

    async function loadData() {
      setIsLoadingData(true);

      try {
        let result: unknown;
        const loader = qs.loader!;

        // Prepare args/sql
        const args = { ...activeParams };

        if (loader.type === "sql") {
          const sql = loader.value.replace(/:(\w+)/g, (_, key) => {
            const value = args[key];
            if (value === null || value === undefined) return "NULL";
            return `'${String(value).replace(/'/g, "''")}'`;
          });
          result = await api.runQuery(projectId, sql, true);
        } else if (loader.type === "edge_function") {
          let functionName = interpolateTemplate(
            loader.name || "",
            activeParams,
          );
          let functionArgs = args as Record<string, unknown>;

          // Attempt to parse as JSON configuration
          const trimmedValue = loader.value.trim();
          if (trimmedValue.startsWith("{")) {
            try {
              // We interpolate parameters into the JSON string first
              // Note: args contains activeParams.
              // We need a string map for interpolation.
              const stringParams: Record<string, string> = {};
              for (const [k, v] of Object.entries(args)) {
                stringParams[k] = String(v);
              }

              // interpolateTemplate needs to be imported or copied?
              // It's not imported in QueryBlock.tsx usually?
              // Check imports.
              // It logic: replace :key with value.

              const interpolated = trimmedValue.replace(
                /:(\w+)/g,
                (match, key) => {
                  return stringParams[key] ?? match;
                },
              );

              const config = JSON.parse(interpolated);

              if (config.body) {
                // If body is present, merge it with existing args
                functionArgs = { ...functionArgs, ...config.body };
              }

              if (config.method) {
                (functionArgs as any).method = config.method;
              }
            } catch (e) {
              console.error("Failed to parse edge function JSON loader", e);
            }
          }

          result = await api.runEdgeFunction(
            projectId,
            functionName,
            functionArgs,
          );
        }

        console.log("loader result:", result);

        let row: Record<string, unknown> | undefined;

        if (Array.isArray(result) && result.length > 0) {
          row = result[0] as Record<string, unknown>;
        } else if (
          result &&
          typeof result === "object" &&
          !Array.isArray(result)
        ) {
          // Handle single object response (common for edge functions returning specific resource)
          row = result as Record<string, unknown>;
        }

        if (row) {
          const loaded: Record<string, unknown> = {};
          for (const field of qs.parameters!) {
            if (row[field.name] !== undefined) {
              loaded[field.name] = row[field.name];
            }
          }
          onFormValuesChange({ ...formValues, ...loaded });
        }
      } catch (err) {
        console.error("Failed to load form data:", err);
      } finally {
        setIsLoadingData(false);
      }
    }

    loadData();
    // Only run on mount or when loader/params change
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    qs.loader?.value,
    qs.loader?.type,
    projectId,
    JSON.stringify(activeParams),
  ]);

  // Error display component - shown when there's an error
  const errorDisplay = qs.error && (
    <div className="p-4">
      <Alert variant="destructive">
        <AlertCircleIcon className="h-4 w-4" />
        <div className="flex items-center gap-8">
          <div className="flex-1">
            <AlertTitle className="mb-1">Failed to run query</AlertTitle>
            <AlertDescription className="text-destructive">
              {qs.error}
            </AlertDescription>
          </div>
          {onFixQuery && (
            <Button
              variant="outline"
              size="sm"
              className="w-fit text-foreground"
              onClick={() => onFixQuery(index, qs.error || undefined)}
              disabled={isProcessingWithAI}
            >
              <Sparkles size={16} strokeWidth={1} />
              {isProcessingWithAI ? "Fixing..." : "Fix with AI"}
            </Button>
          )}
        </div>
      </Alert>
    </div>
  );

  // Show loading state when loading data for forms
  if (isLoadingData) {
    return (
      <div className="flex-1 flex flex-col border-b min-h-[300px] items-center justify-center">
        <Loader2 className="size-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col border-b min-h-[300px]">
      <div className="flex-1 flex flex-col overflow-hidden gap-0">
        <div className="group shrink-0 border-b bg-muted/20 flex flex-col relative group">
          <QueryInput
            mode={mode}
            setMode={setMode}
            showToggle={!!(qs.parameters && qs.parameters.length > 0)}
            onRun={() => onRunQuery(index)}
            isLoading={isLoading}
            isProcessingWithAI={isProcessingWithAI}
            canRemove={canRemove}
            onRemove={onRemove}
          >
            <div className="overflow-auto min-h-[150px] h-full">
              {mode === "form" && qs.parameters ? (
                <SqlFormArea
                  formConfig={{ fields: qs.parameters }}
                  formValues={formValues}
                  onFormValuesChange={onFormValuesChange}
                  onSubmit={() => onRunQuery(index)}
                />
              ) : (
                <SqlQueryArea
                  sql={qs.source?.value ?? ((qs as any).sql || "")}
                  setSql={(newSql) => onSqlChange(index, newSql)}
                  handleKeyDown={(e) => {
                    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                      e.preventDefault();
                      onRunQuery(index);
                    }
                  }}
                  // If we want to indicate the source type, we might add a badge or label in SqlQueryArea
                  // For now treating edge function name as "SQL" text for editing purposes is acceptable default
                />
              )}
            </div>
          </QueryInput>
        </div>

        {/* Error Display - shown at QueryBlock level when no results area */}
        {qs.resultsConfig === null && errorDisplay}

        {/* Results Area */}
        {qs.resultsConfig !== null && (
          <SqlResultsArea
            error={qs.error}
            results={qs.results}
            displayColumns={qs.displayColumns}
            handleDataChange={(newData) => onResultsChange(index, newData)}
            rowActions={qs.rowActions}
            onRowAction={onRowAction}
            chart={qs.resultsConfig === "chart" ? qs.chart : undefined}
            onFixQuery={
              onFixQuery ? (err) => onFixQuery(index, err) : undefined
            }
            isProcessingWithAI={isProcessingWithAI}
          />
        )}
      </div>
    </div>
  );
}
