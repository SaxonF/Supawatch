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
          rows={5}
          placeholder="Enter SQL or describe what you want in plain English..."
          className="block rounded-none p-5 !bg-transparent focus:!bg-muted/25 font-mono text-muted-foreground focus:text-foreground w-full border-none focus-visible:ring-0"
          spellCheck={false}
        />
        <div className="absolute bottom-3 right-3 flex items-center gap-2">
          {isProcessingWithAI && (
            <div className="rounded-full bg-primary/20 text-primary py-1.5 px-3 text-sm font-medium flex items-center gap-2">
              <span>Generating with AI...</span>
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
                strokeWidth={1}
                className="animate-pulse text-primary"
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
