import Spreadsheet from "react-spreadsheet";
import { SpreadsheetData } from "./types";

interface SqlResultsAreaProps {
  error: string | null;
  results: SpreadsheetData;
  displayColumns: string[];
  handleDataChange: (newData: SpreadsheetData) => void;
}

export function SqlResultsArea({
  error,
  results,
  displayColumns,
  handleDataChange,
}: SqlResultsAreaProps) {
  return (
    <div className="select-none flex-1 overflow-auto [scrollbar-width:none] [scrollbar-height:none] [&::-webkit-scrollbar]:hidden">
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
            darkMode={true}
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
  );
}
