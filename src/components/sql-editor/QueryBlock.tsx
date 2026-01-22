import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertCircleIcon, Sparkles } from "lucide-react";
import { useState } from "react";
import { Button } from "../ui/button";
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
}: QueryBlockProps) {
  const [mode, setMode] = useState<"form" | "sql">(
    qs.parameters?.length ? "form" : "sql",
  );

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

  return (
    <div className="flex-1 flex flex-col border-b min-h-[300px]">
      <div className="flex-1 flex flex-col overflow-hidden gap-0">
        <div className="shrink-0 border-b bg-muted/20 flex flex-col">
          {/* Header / Tabs if parameters exist */}
          {qs.parameters && qs.parameters.length > 0 && (
            <div className="flex items-center px-4 border-b bg-background">
              <div className="flex gap-4">
                <button
                  onClick={() => setMode("form")}
                  className={`py-2 text-sm font-medium border-b-2 ${
                    mode === "form"
                      ? "border-primary text-primary"
                      : "border-transparent text-muted-foreground hover:text-foreground"
                  }`}
                >
                  Form
                </button>
                <button
                  onClick={() => setMode("sql")}
                  className={`py-2 text-sm font-medium border-b-2 ${
                    mode === "sql"
                      ? "border-primary text-primary"
                      : "border-transparent text-muted-foreground hover:text-foreground"
                  }`}
                >
                  SQL
                </button>
              </div>
            </div>
          )}

          {/* Content Area (Form or SQL) */}
          <div className="overflow-auto">
            {mode === "form" && qs.parameters ? (
              <SqlFormArea
                projectId={projectId}
                sql={qs.sql}
                formConfig={{ fields: qs.parameters }}
                loadQuery={qs.loadQuery}
                params={activeParams}
                formValues={formValues}
                onFormValuesChange={onFormValuesChange}
                onSubmit={() => onRunQuery(index)}
                isSubmitting={isLoading}
                isProcessingWithAI={isProcessingWithAI}
              />
            ) : (
              <SqlQueryArea
                sql={qs.sql}
                setSql={(newSql) => onSqlChange(index, newSql)}
                runQuery={() => onRunQuery(index)}
                isLoading={isLoading}
                isProcessingWithAI={isProcessingWithAI}
                handleKeyDown={(e) => {
                  if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                    e.preventDefault();
                    onRunQuery(index);
                  }
                }}
              />
            )}
          </div>
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
