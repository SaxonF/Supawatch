import { ask } from "@tauri-apps/plugin-dialog";
import { AlertCircle, CloudUpload, FileDiff, RefreshCw, X } from "lucide-react";
import { useEffect, useState } from "react";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { vscDarkPlus } from "react-syntax-highlighter/dist/esm/styles/prism";
import * as api from "../api";
import type { DiffResponse } from "../types";
import { Button } from "./ui/button";

interface DiffSidebarProps {
  projectId: string;
  onClose: () => void;
  onSuccess: () => void;
}

export function DiffSidebar({
  projectId,
  onClose,
  onSuccess,
}: DiffSidebarProps) {
  const [diff, setDiff] = useState<DiffResponse | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isPushing, setIsPushing] = useState(false);
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
      const data = await api.getProjectDiff(projectId);
      setDiff(data);
    } catch (err) {
      console.error("Failed to load diff:", err);
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const handlePush = async () => {
    if (!diff) return;

    // Standard confirmation for any push
    if (!diff.is_destructive) {
      // Determine if we should ask for confirmation even for non-destructive?
      // ProjectHeader logic asks "No changes" or "Success".
      // Here we know there are changes (unless empty).
      // Let's just push.
    } else {
      const confirmed = await ask(
        `Destructive changes detected!\n\n${diff.summary}\n\nDo you want to force push these changes?`,
        {
          title: "Destructive Changes Detected",
          kind: "warning",
          okLabel: "Push Changes",
          cancelLabel: "Cancel",
        }
      );
      if (!confirmed) return;
    }

    setIsPushing(true);
    try {
      await api.pushProject(projectId, diff.is_destructive);
      await ask("Schema changes pushed successfully", {
        title: "Success",
        kind: "info",
      });
      loadDiff(); // Refresh to show empty
      onSuccess();
    } catch (err) {
      console.error("Failed to push project:", err);
      // Check if it's the confirmation needed error (shouldn't happen if we trust diff.is_destructive, but good fallback)
      const errorMsg = String(err);
      if (errorMsg.startsWith("CONFIRMATION_NEEDED:")) {
        // This path might happen if backend detects something our diff check missed, or race condition
        const summary = errorMsg.replace("CONFIRMATION_NEEDED:", "");
        const confirmed = await ask(
          `Destructive changes detected!\n\n${summary}\n\nDo you want to force push these changes?`,
          {
            title: "Destructive Changes Detected",
            kind: "warning",
            okLabel: "Push Changes",
            cancelLabel: "Cancel",
          }
        );

        if (confirmed) {
          try {
            await api.pushProject(projectId, true);
            await ask("Schema changes pushed successfully", {
              title: "Success",
              kind: "info",
            });
            loadDiff();
            onSuccess();
          } catch (retryErr) {
            await ask("Failed to push project: " + String(retryErr), {
              title: "Error",
              kind: "error",
            });
          }
        }
      } else {
        await ask("Failed to push project: " + String(err), {
          title: "Error",
          kind: "error",
        });
      }
    } finally {
      setIsPushing(false);
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex items-center justify-between px-5 py-3 border-b shrink-0 bg-background">
        <h2 className="font-semibold flex items-center gap-2">
          <FileDiff size={18} />
          Schema Diff
        </h2>
        <div className="flex items-center gap-2">
          <Button
            variant="ghost"
            size="icon"
            onClick={loadDiff}
            title="Refresh diff"
            disabled={isLoading || isPushing}
          >
            <RefreshCw size={16} className={isLoading ? "animate-spin" : ""} />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            onClick={onClose}
            title="Close"
            disabled={isPushing}
          >
            <X size={16} />
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-0 bg-[#1e1e1e]">
        {isLoading && !diff ? (
          <div className="flex items-center justify-center h-full text-muted-foreground bg-background">
            Loading diff...
          </div>
        ) : error ? (
          <div className="flex flex-col items-center justify-center h-full p-4 text-center bg-background">
            <p className="text-destructive mb-2">Failed to load diff</p>
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
        ) : !diff ||
          (diff.migration_sql.trim() === "" && !diff.is_destructive) ? (
          <div className="flex flex-col items-center justify-center h-full text-muted-foreground p-4 text-center bg-background">
            <p>No changes detected</p>
            <p className="text-sm mt-1">Local schema matches remote</p>
          </div>
        ) : (
          <div className="flex flex-col h-full">
            <div className="flex-1 overflow-auto">
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
            </div>

            {diff.is_destructive && (
              <div className="p-4 bg-yellow-500/10 border-t border-yellow-500/20">
                <div className="flex items-start gap-3">
                  <AlertCircle className="text-yellow-500 mt-0.5" size={16} />
                  <div className="text-sm">
                    <p className="font-medium text-yellow-500">
                      Destructive Changes Detected
                    </p>
                    <p className="text-muted-foreground mt-1">
                      This update involves data loss. Please review carefully.
                    </p>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      <div className="p-4 border-t shrink-0 bg-background">
        <Button
          className="w-full gap-2"
          onClick={handlePush}
          disabled={
            isLoading ||
            isPushing ||
            !diff ||
            (diff.migration_sql.trim() === "" && !diff.is_destructive)
          }
          variant={diff?.is_destructive ? "destructive" : "default"}
        >
          <CloudUpload size={16} />
          {diff?.is_destructive ? "Force Push Changes" : "Push Changes"}
        </Button>
      </div>
    </div>
  );
}
