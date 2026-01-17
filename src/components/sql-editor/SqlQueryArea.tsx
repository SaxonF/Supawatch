import { Play } from "lucide-react";
import { Button } from "../ui/button";
import { Textarea } from "../ui/textarea";

interface SqlQueryAreaProps {
  sql: string;
  setSql: (sql: string) => void;
  runQuery: () => void;
  isLoading: boolean;
  handleKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void;
}

export function SqlQueryArea({
  sql,
  setSql,
  runQuery,
  isLoading,
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
          placeholder="SELECT * FROM your_table"
          className="block rounded-none p-5 !bg-transparent focus:!bg-muted/25 font-mono text-muted-foreground focus:text-foreground w-full border-none focus-visible:ring-0"
          spellCheck={false}
        />
        <Button
          onClick={runQuery}
          disabled={isLoading || !sql.trim()}
          size="icon"
          variant={"secondary"}
          className="absolute bottom-3 right-3 rounded-full shadow-lg hover:scale-105 transition-transform"
          title="Run query (Cmd+Enter)"
        >
          <Play
            size={16}
            strokeWidth={1.5}
            className={isLoading ? "animate-spin" : "ml-0.5"}
          />
        </Button>
      </div>
    </div>
  );
}
