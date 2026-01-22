import { Database, FileText, Play, Sparkles } from "lucide-react";
import { Button } from "../ui/button";

interface QueryInputProps {
  mode: "form" | "sql";
  setMode: (mode: "form" | "sql") => void;
  showToggle: boolean;
  onRun: () => void;
  isLoading: boolean;
  isProcessingWithAI?: boolean;
  children: React.ReactNode;
}

export function QueryInput({
  mode,
  setMode,
  showToggle,
  onRun,
  isLoading,
  isProcessingWithAI,
  children,
}: QueryInputProps) {
  return (
    <div className="relative group flex-1 flex flex-col min-h-0">
      {/* Content Area */}
      <div className="flex-1 flex flex-col min-h-0 relative">
        {children}

        {/* Floating Action Buttons */}
        <div className="absolute bottom-4 right-4 flex items-center gap-3 z-10">
          {/* Toggle Buttons (only if params exist) */}
          {showToggle && (
            <div className="rounded rounded-full flex items-center gap-2">
              <Button
                size="icon-sm"
                variant={mode === "sql" ? "secondary" : "outline"}
                onClick={() => setMode("sql")}
                title="Switch to SQL"
              >
                <Database size={16} />
              </Button>

              <Button
                size="icon-sm"
                variant={mode === "form" ? "secondary" : "outline"}
                onClick={() => setMode("form")}
                title="Switch to Form"
              >
                <FileText size={16} />
              </Button>
            </div>
          )}

          {/* Main Run Button */}
          <Button
            onClick={onRun}
            disabled={isLoading}
            size="sm"
            title="Run (Cmd+Enter)"
            variant={showToggle ? "default" : "outline"}
          >
            {isProcessingWithAI ? (
              <Sparkles size={16} strokeWidth={1} className="animate-pulse" />
            ) : (
              <Play
                size={16}
                strokeWidth={1}
                fill="currentColor"
                className={isLoading ? "animate-pulse ml-0.5" : "ml-0.5"}
              />
            )}
            Run
          </Button>
        </div>
      </div>
    </div>
  );
}
