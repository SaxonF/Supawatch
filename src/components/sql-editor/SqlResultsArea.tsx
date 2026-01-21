import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { ChartSpec, RowAction } from "@/specs/types";
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
  chart?: ChartSpec;
}

import {
  ChartConfig,
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
} from "@/components/ui/chart";
import * as React from "react";
import { Bar, BarChart, CartesianGrid, XAxis } from "recharts";

export function SqlResultsArea({
  error,
  results,
  displayColumns,
  handleDataChange,
  onFixQuery,
  isProcessingWithAI = false,
  rowActions,
  onRowAction,
  chart,
}: SqlResultsAreaProps) {
  // Generate internal chart config from ChartSpec
  const internalChartConfig = React.useMemo<ChartConfig | null>(() => {
    if (!chart) return null;

    // Assign colors from chart css variables
    const config: ChartConfig = {};
    chart.yAxis.forEach((axis, idx) => {
      // Rotate through 5 chart colors
      const colorIndex = (idx % 5) + 1;
      config[axis.name] = {
        label: axis.label || axis.name,
        color: `var(--chart-${colorIndex})`,
      };
    });

    return config;
  }, [chart]);

  // Transform results for chart if needed
  const chartData = React.useMemo(() => {
    if (!chart || !results.length) return [];

    return results.map((row) => {
      const rowData: Record<string, any> = {};
      displayColumns.forEach((col, idx) => {
        const cellValue = row[idx]?.value;
        // Try to parse numbers for chart values
        const numValue = Number(cellValue);
        rowData[col] =
          !isNaN(numValue) && cellValue !== "" ? numValue : cellValue;
      });
      return rowData;
    });
  }, [results, displayColumns, chart]);

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
        <div className="sql-results-spreadsheet h-full flex flex-col">
          {chart && internalChartConfig ? (
            <div className="flex-1 p-6">
              <ChartContainer
                config={internalChartConfig}
                className="h-full w-full"
              >
                <BarChart accessibilityLayer data={chartData}>
                  <CartesianGrid vertical={false} />
                  <XAxis
                    dataKey={chart.xAxis.name}
                    tickLine={false}
                    tickMargin={10}
                    axisLine={false}
                    tickFormatter={(value) => {
                      // Attempt to format dates if the value looks like a date
                      const date = new Date(value);
                      if (!isNaN(date.getTime())) {
                        return date.toLocaleDateString("en-US", {
                          month: "short",
                          day: "numeric",
                        });
                      }
                      return value;
                    }}
                  />
                  <ChartTooltip
                    cursor={false}
                    content={<ChartTooltipContent hideLabel {...({} as any)} />}
                  />
                  {chart.yAxis.map((axis) => (
                    <Bar
                      key={axis.name}
                      dataKey={axis.name}
                      fill={`var(--color-${axis.name})`}
                      radius={8}
                    />
                  ))}
                </BarChart>
              </ChartContainer>
            </div>
          ) : rowActions && rowActions.length > 0 ? (
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
