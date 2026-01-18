import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { AlertCircleIcon, Sparkles } from "lucide-react";
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
}

export function SqlResultsArea({
  error,
  results,
  displayColumns,
  handleDataChange,
  onFixQuery,
  isProcessingWithAI = false,
}: SqlResultsAreaProps) {
  return (
    <div className="select-none flex-1 overflow-auto [scrollbar-width:none] [scrollbar-height:none] [&::-webkit-scrollbar]:hidden">
      {error ? (
        <div className="p-4">
          <Alert variant="destructive">
            <AlertCircleIcon className="h-4 w-4" />
            <div className="flex items-center">
              <div>
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
          <Spreadsheet
            data={results}
            darkMode={true}
            columnLabels={displayColumns}
            onChange={handleDataChange}
          />
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
