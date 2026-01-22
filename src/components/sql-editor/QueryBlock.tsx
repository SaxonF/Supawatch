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

  // Load existing data if loadQuery is defined (for edit forms)
  useEffect(() => {
    if (!qs.loadQuery || !qs.parameters?.length) return;

    async function loadData() {
      setIsLoadingData(true);

      try {
        // Interpolate params into loadQuery
        const sql = qs.loadQuery!.replace(/:(\w+)/g, (_, key) => {
          const value = activeParams[key];
          if (value === null || value === undefined) return "NULL";
          return `'${String(value).replace(/'/g, "''")}'`;
        });

        const result = await api.runQuery(projectId, sql, true);

        if (Array.isArray(result) && result.length > 0) {
          const row = result[0] as Record<string, unknown>;
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
    // Only run on mount or when loadQuery/params change
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [qs.loadQuery, projectId, JSON.stringify(activeParams)]);

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
                  sql={qs.sql}
                  setSql={(newSql) => onSqlChange(index, newSql)}
                  handleKeyDown={(e) => {
                    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                      e.preventDefault();
                      onRunQuery(index);
                    }
                  }}
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
