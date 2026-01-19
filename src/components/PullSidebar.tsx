import { ask } from "@tauri-apps/plugin-dialog";
import { CloudDownload, RefreshCw, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { vscDarkPlus } from "react-syntax-highlighter/dist/esm/styles/prism";
import * as api from "../api";
import type { PullDiffResponse } from "../types";
import { Button } from "./ui/button";

interface PullSidebarProps {
  projectId: string;
  onClose: () => void;
  onSuccess: () => void;
}

export function PullSidebar({
  projectId,
  onClose,
  onSuccess,
}: PullSidebarProps) {
  const [diff, setDiff] = useState<PullDiffResponse | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isPulling, setIsPulling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (projectId) {
      loadDiff();
    }
  }, [projectId]);

  const loadDiff = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const data = await api.getPullDiff(projectId);
      setDiff(data);
    } catch (err) {
      console.error("Failed to load pull preview:", err);
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const handlePull = async () => {
    // Confirmation is now implicit in the action of clicking "Pull" in this sidebar?
    // User said "sidebar action will trigger this (same as existing confirmation prompt which we wont need anymore)".
    // Use a safety confirmation still? "Overwrite local changes... This cannot be undone."
    // Yes, explicit confirmation inside sidebar button click is safer.

    const confirmed = await ask(
      `Overwrite local changes? This cannot be undone.`,
      {
        title: "Confirm Pull",
        kind: "warning",
        okLabel: "Overwrite",
        cancelLabel: "Cancel",
      }
    );

    if (!confirmed) return;

    setIsPulling(true);
    try {
      await api.pullProject(projectId);
      await ask("Project pulled successfully", {
        title: "Success",
        kind: "info",
      });
      onSuccess();
    } catch (err) {
      console.error("Failed to pull project:", err);
      await ask("Failed to pull project: " + String(err), {
        title: "Error",
        kind: "error",
      });
    } finally {
      setIsPulling(false);
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex items-center justify-between px-5 py-3 border-b shrink-0 bg-background">
        <h2 className="font-semibold flex items-center gap-2">
          <CloudDownload
            size={16}
            strokeWidth={1}
            className="text-muted-foreground"
          />
          Remote Content
        </h2>
        <div className="flex items-center gap-0">
          <Button
            variant="ghost"
            size="icon"
            onClick={loadDiff}
            title="Refresh"
            disabled={isLoading || isPulling}
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
            disabled={isPulling}
          >
            <X size={16} strokeWidth={1} />
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-0 bg-muted/25">
        {isLoading && !diff ? (
          <div className="flex items-center justify-center h-full text-muted-foreground bg-background">
            Loading preview...
          </div>
        ) : error ? (
          <div className="flex flex-col items-center justify-center h-full p-4 text-center bg-background">
            <p className="text-destructive mb-2">Failed to load preview</p>
            <p className="text-sm text-muted-foreground">{error}</p>
            <Button
              variant="outline"
              size="sm"
              onClick={loadDiff}
              className="mt-4"
            >
              Try Again
            </Button>
          </div>
        ) : !diff ? (
          <div className="flex items-center justify-center h-full text-muted-foreground bg-background">
            No content loaded
          </div>
        ) : (
          <div className="flex flex-col h-full">
            {diff.edge_functions.length > 0 && (
              <div className="p-4 border-b shrink-0">
                <h3 className="text-xs text-muted-foreground uppercase tracking-wider mb-2 font-mono">
                  Remote Edge Functions ({diff.edge_functions.length})
                </h3>
                <div className="flex items-center gap-2 flex-wrap">
                  {diff.edge_functions.map((func) => (
                    <div
                      key={func.slug}
                      className="flex items-center gap-2 text-sm rounded-full py-2 px-3 bg-muted"
                    >
                      <div className="w-1.5 h-1.5 rounded-full bg-blue-500 shrink-0" />
                      <span className="font-mono text-xs">{func.name}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
            <div className="flex-1 overflow-auto">
              <h3 className="text-xs text-muted-foreground uppercase tracking-wider mb-2 font-mono p-4 pb-0">
                Remote Schema
              </h3>
              {diff.migration_sql.trim() !== "" ? (
                <SyntaxHighlighter
                  language="sql"
                  style={vscDarkPlus}
                  customStyle={{
                    margin: 0,
                    padding: "1rem",
                    background: "transparent",
                    fontSize: "12px",
                    height: "100%",
                  }}
                  showLineNumbers={true}
                  wrapLines={true}
                >
                  {diff.migration_sql}
                </SyntaxHighlighter>
              ) : (
                <div className="flex items-center justify-center h-full text-muted-foreground text-sm p-8">
                  No schema content found
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      <div className="p-4 border-t shrink-0 bg-background">
        <Button
          className="w-full gap-2"
          onClick={handlePull}
          disabled={isLoading || isPulling || !diff}
          // Variant default since it's a primary action, or destructive to allow caution?
          // Pulling overwrites, so maybe destructive/warning flavor?
          // But user wants to pull. Let's keep it default but maybe add a warning icon/text.
          variant="default"
        >
          <CloudDownload size={16} />
          Overwrite Local with Remote
        </Button>
      </div>
    </div>
  );
}
