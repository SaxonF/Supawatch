import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { RowAction } from "@/specs/types";
import { AlertCircleIcon, MoreHorizontal, Sparkles } from "lucide-react";
import Spreadsheet from "react-spreadsheet";
import { Button } from "../ui/button";
import { SpreadsheetData } from "./types";

interface SqlResultsAreaProps {
  error: string | null;
  results: SpreadsheetData;
  displayColumns: string[];
  handleDataChange: (newData: SpreadsheetData) => void;
  onFixQuery?: () => void;
  isProcessingWithAI?: boolean;
  rowActions?: RowAction[];
  onRowAction?: (action: RowAction, row: Record<string, any>) => void;
}

export function SqlResultsArea({
  error,
  results,
  displayColumns,
  handleDataChange,
  onFixQuery,
  isProcessingWithAI = false,
  rowActions,
  onRowAction,
}: SqlResultsAreaProps) {
  return (
    <div className="select-none flex-1 overflow-auto [scrollbar-width:none] [scrollbar-height:none] [&::-webkit-scrollbar]:hidden">
      {error ? (
        <div className="p-4">
          <Alert variant="destructive">
            <AlertCircleIcon className="h-4 w-4" />
            <div className="flex items-center gap-8">
              <div className="flex-1">
                <AlertTitle className="mb-1">Failed to run query</AlertTitle>
                <AlertDescription className="text-destructive">
                  {error}
                </AlertDescription>
              </div>
              {onFixQuery && (
                <Button
                  variant="outline"
                  size="sm"
                  className="w-fit text-foreground"
                  onClick={onFixQuery}
                  disabled={isProcessingWithAI}
                >
                  <Sparkles size={16} strokeWidth={1} />
                  {isProcessingWithAI ? "Fixing..." : "Fix with AI"}
                </Button>
              )}
            </div>
          </Alert>
        </div>
      ) : results.length > 0 ? (
        <div className="sql-results-spreadsheet">
          {rowActions && rowActions.length > 0 ? (
            <div className="flex-1 overflow-auto">
              <table className="w-full border-collapse">
                <thead className="bg-background sticky top-0 z-10">
                  <tr>
                    {displayColumns.map((col) => (
                      <th
                        key={col}
                        className="text-left p-3 font-mono text-xs uppercase font-normal border border-muted-border whitespace-nowrap bg-background text-muted-foreground/75"
                      >
                        {col}
                      </th>
                    ))}
                    <th className="w-10 border border-[var(--muted-border)] bg-background"></th>
                  </tr>
                </thead>
                <tbody className="text-muted-foreground">
                  {results.map((row, rowIdx) => (
                    <tr
                      key={rowIdx}
                      className="hover:text-foreground transition-colors group"
                    >
                      {row.map((cell, cellIdx) => (
                        <td
                          key={cellIdx}
                          className="p-3 border border-[var(--muted-border)] max-w-xs truncate"
                          title={cell?.value}
                        >
                          {cell?.value}
                        </td>
                      ))}
                      <td className="px-2 border border-[var(--muted-border)]">
                        <DropdownMenu>
                          <DropdownMenuTrigger asChild>
                            <Button
                              variant="ghost"
                              size="icon-sm"
                              className="opacity-0 group-hover:opacity-100 transition-opacity data-[state=open]:opacity-100"
                            >
                              <MoreHorizontal className="size-4" />
                            </Button>
                          </DropdownMenuTrigger>
                          <DropdownMenuContent align="end">
                            {rowActions.map((action, actionIdx) => (
                              <DropdownMenuItem
                                key={actionIdx}
                                className={
                                  action.variant === "destructive"
                                    ? "text-destructive focus:text-destructive"
                                    : ""
                                }
                                onClick={() => {
                                  // Convert row array to object for params
                                  const rowObj: Record<string, string> = {};
                                  displayColumns.forEach((col, i) => {
                                    rowObj[col] =
                                      results[rowIdx][i]?.value || "";
                                  });
                                  onRowAction?.(action, rowObj);
                                }}
                              >
                                {action.label}
                              </DropdownMenuItem>
                            ))}
                          </DropdownMenuContent>
                        </DropdownMenu>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          ) : (
            <Spreadsheet
              data={results}
              darkMode={true}
              columnLabels={displayColumns}
              onChange={handleDataChange}
            />
          )}
        </div>
      ) : (
        <div className="flex flex-col items-center justify-center h-full">
          <p>No results</p>
          <p className="mt-1 text-muted-foreground">
            Run a query to see results here
          </p>
        </div>
      )}
    </div>
  );
}
