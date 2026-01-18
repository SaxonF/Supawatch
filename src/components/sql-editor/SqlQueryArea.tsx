import { Play, Sparkles } from "lucide-react";
import { Button } from "../ui/button";
import { Textarea } from "../ui/textarea";

interface SqlQueryAreaProps {
  sql: string;
  setSql: (sql: string) => void;
  runQuery: () => void;
  isLoading: boolean;
  isProcessingWithAI?: boolean;
  handleKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void;
}

export function SqlQueryArea({
  sql,
  setSql,
  runQuery,
  isLoading,
  isProcessingWithAI = false,
  handleKeyDown,
}: SqlQueryAreaProps) {
  return (
    <div className="shrink-0 border-b relative group">
      <div className="relative">
        <Textarea
          value={sql}
          onChange={(e) => setSql(e.target.value)}
          onKeyDown={handleKeyDown}
          rows={10}
          placeholder="Enter SQL or describe what you want in plain English..."
          className="block rounded-none p-5 !bg-transparent focus:!bg-muted/25 font-mono text-muted-foreground focus:text-foreground w-full border-none focus-visible:ring-0"
          spellCheck={false}
        />
        <div className="absolute bottom-3 right-3 flex items-center gap-2">
          {isProcessingWithAI && (
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground bg-muted/50 px-2 py-1 rounded-full">
              <Sparkles size={12} className="animate-pulse text-amber-500" />
              <span>Converting with AI...</span>
            </div>
          )}
          <Button
            onClick={runQuery}
            disabled={isLoading || !sql.trim()}
            size="icon"
            variant={"secondary"}
            className="rounded-full shadow-lg hover:scale-105 transition-transform"
            title="Run query (Cmd+Enter)"
          >
            {isProcessingWithAI ? (
              <Sparkles
                size={16}
                strokeWidth={1.5}
                className="animate-pulse text-amber-500"
              />
            ) : (
              <Play
                size={16}
                strokeWidth={1.5}
                className={isLoading ? "animate-spin" : "ml-0.5"}
              />
            )}
          </Button>
        </div>
      </div>
    </div>
  );
}
