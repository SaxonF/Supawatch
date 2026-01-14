import { Play } from "lucide-react";
import { useState } from "react";
import Spreadsheet from "react-spreadsheet";
import * as api from "../api";
import { Button } from "./ui/button";

interface SqlEditorProps {
  projectId: string;
}

type CellValue = { value: string } | undefined;

export function SqlEditor({ projectId }: SqlEditorProps) {
  const [sql, setSql] = useState("SELECT * FROM ");
  const [results, setResults] = useState<CellValue[][]>([]);
  const [columns, setColumns] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const runQuery = async () => {
    if (!sql.trim()) return;

    setIsLoading(true);
    setError(null);

    try {
      const result = await api.runQuery(projectId, sql);

      if (Array.isArray(result) && result.length > 0) {
        // Extract column names from first row
        const cols = Object.keys(result[0]);
        setColumns(cols);

        // Convert to spreadsheet format
        const data: CellValue[][] = result.map((row: Record<string, unknown>) =>
          cols.map((col) => {
            const value = row[col];
            return {
              value: value === null ? "NULL" : String(value),
            };
          })
        );

        setResults(data);
      } else {
        setColumns([]);
        setResults([]);
      }
    } catch (err) {
      console.error("Query failed:", err);
      setError(typeof err === "string" ? err : String(err));
      setResults([]);
      setColumns([]);
    } finally {
      setIsLoading(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      e.preventDefault();
      runQuery();
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden">
      {/* SQL Input Area */}
      <div className="shrink-0 p-4 border-b">
        <div className="flex gap-2">
          <textarea
            value={sql}
            onChange={(e) => setSql(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="SELECT * FROM your_table"
            className="flex-1 min-h-[100px] p-3 bg-muted rounded-lg border border-input font-mono text-sm resize-y focus:outline-none focus:ring-2 focus:ring-ring"
            spellCheck={false}
          />
          <Button
            onClick={runQuery}
            disabled={isLoading || !sql.trim()}
            className="shrink-0 self-start"
            title="Run query (Cmd+Enter)"
          >
            <Play size={16} className="mr-2" />
            {isLoading ? "Running..." : "Run"}
          </Button>
        </div>
        <p className="text-xs text-muted-foreground mt-2">
          Press Cmd+Enter to run query
        </p>
      </div>

      {/* Results Area */}
      <div className="flex-1 overflow-auto p-4">
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
              columnLabels={columns}
              onChange={setResults}
            />
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
            <p>No results</p>
            <p className="text-sm mt-1">Run a query to see results here</p>
          </div>
        )}
      </div>
    </div>
  );
}
