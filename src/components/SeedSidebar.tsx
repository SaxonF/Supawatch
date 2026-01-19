import { ask } from "@tauri-apps/plugin-dialog";
import { Play, RefreshCw, Sprout, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { vscDarkPlus } from "react-syntax-highlighter/dist/esm/styles/prism";
import * as api from "../api";
import { Button } from "./ui/button";

interface SeedSidebarProps {
  projectId: string;
  onClose: () => void;
}

export function SeedSidebar({ projectId, onClose }: SeedSidebarProps) {
  const [content, setContent] = useState<string | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isRunning, setIsRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (projectId) {
      loadSeeds();
    }
  }, [projectId]);

  const loadSeeds = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await api.getSeedContent(projectId);
      setContent(data);
    } catch (err) {
      console.error("Failed to load seeds:", err);
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const handleRun = async () => {
    setIsRunning(true);
    try {
      const result = await api.runSeeds(projectId);
      await ask(result, {
        title: "Seeds Executed",
        kind: "info",
      });
    } catch (err) {
      console.error("Failed to run seeds:", err);
      await ask("Failed to run seeds: " + String(err), {
        title: "Error",
        kind: "error",
      });
    } finally {
      setIsRunning(false);
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex items-center justify-between px-5 py-3 border-b shrink-0 bg-background">
        <h2 className="font-semibold flex items-center gap-2">
          <Sprout size={16} strokeWidth={1} className="text-muted-foreground" />
          Seed Files
        </h2>
        <div className="flex items-center gap-0">
          <Button
            variant="ghost"
            size="icon"
            onClick={loadSeeds}
            title="Refresh seeds"
            disabled={isLoading || isRunning}
          >
            <RefreshCw
              size={16}
              strokeWidth={1}
              className={isLoading ? "animate-spin" : ""}
            />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            onClick={onClose}
            title="Close"
            disabled={isRunning}
          >
            <X size={16} strokeWidth={1} />
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-0 bg-muted/25">
        {isLoading && !content ? (
          <div className="flex items-center justify-center h-full text-muted-foreground bg-background">
            Loading seed files...
          </div>
        ) : error ? (
          <div className="flex flex-col items-center justify-center h-full p-4 text-center bg-background">
            <p className="text-destructive mb-2">Failed to load seeds</p>
            <p className="text-sm text-muted-foreground">{error}</p>
            <Button
              variant="outline"
              size="sm"
              onClick={loadSeeds}
              className="mt-4"
            >
              Try Again
            </Button>
          </div>
        ) : !content || content.trim().startsWith("-- No seed") ? (
          <div className="flex flex-col items-center justify-center h-full p-4 text-center bg-background">
            <p>No seed files found</p>
            <p className="text-sm mt-1 text-muted-foreground">
              Add .sql files to supabase/seed/ to see them here
            </p>
          </div>
        ) : (
          <SyntaxHighlighter
            language="sql"
            style={vscDarkPlus}
            customStyle={{
              margin: 0,
              padding: "1rem",
              background: "transparent",
              fontSize: "12px",
              height: "100%",
              width: "100%",
            }}
            showLineNumbers={true}
            wrapLines={true}
          >
            {content}
          </SyntaxHighlighter>
        )}
      </div>

      <div className="p-4 border-t shrink-0 bg-background">
        <Button
          className="w-full gap-2"
          onClick={handleRun}
          disabled={
            isLoading ||
            isRunning ||
            !content ||
            content.trim().startsWith("-- No seed")
          }
        >
          <Play size={16} />
          Run Scripts
        </Button>
      </div>
    </div>
  );
}
