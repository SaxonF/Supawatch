import { Textarea } from "../ui/textarea";

interface SqlQueryAreaProps {
  sql: string;
  setSql: (sql: string) => void;
  handleKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void;
}

export function SqlQueryArea({
  sql,
  setSql,
  handleKeyDown,
}: SqlQueryAreaProps) {
  return (
    <div className="h-full relative group bg-gradient-to-br from-transparent to-muted/20">
      <div className="relative h-full">
        <Textarea
          value={sql}
          autoFocus={sql === ""}
          onChange={(e) => setSql(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Enter SQL or describe what you want in plain English..."
          className="block leading-relaxed rounded-none p-5 !bg-transparent focus:!bg-muted/25 font-mono text-muted-foreground focus:text-foreground w-full h-full border-none focus-visible:ring-0 resize-none"
          spellCheck={false}
        />
      </div>
    </div>
  );
}
