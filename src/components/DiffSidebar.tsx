import { ask } from "@tauri-apps/plugin-dialog";
import {
  AlertCircle,
  AlertTriangle,
  CheckCircle,
  CloudUpload,
  Copy,
  FileDiff,
  RefreshCw,
  X,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { vscDarkPlus } from "react-syntax-highlighter/dist/esm/styles/prism";
import * as api from "../api";
import type { DiffResponse, EdgeFunctionDeploymentResult } from "../types";
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
  const [deploymentResults, setDeploymentResults] = useState<
    EdgeFunctionDeploymentResult[] | null
  >(null);

  useEffect(() => {
    if (projectId) {
      loadDiff();
    }
  }, [projectId]);

  const loadDiff = async () => {
    setIsLoading(true);
    setError(null);
    setDeploymentResults(null);
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
        },
      );
      if (!confirmed) return;
    }

    setIsPushing(true);
    setDeploymentResults(null);
    try {
      const response = await api.pushProject(projectId, diff.is_destructive);

      setDeploymentResults(response.edge_function_results);

      const hasErrors = response.edge_function_results.some(
        (r) => r.status === "error",
      );

      if (hasErrors) {
        await ask(
          "Some edge functions failed to deploy. Please check the results.",
          {
            title: "Deployment Warning",
            kind: "warning",
          },
        );
        // Do NOT close or refresh immediately so user can see errors
      } else {
        await ask("Schema changes pushed successfully", {
          title: "Success",
          kind: "info",
        });
        loadDiff(); // Refresh to show empty
        onSuccess();
      }
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
          },
        );

        if (confirmed) {
          try {
            const response = await api.pushProject(projectId, true);
            setDeploymentResults(response.edge_function_results);

            const hasErrors = response.edge_function_results.some(
              (r) => r.status === "error",
            );

            if (hasErrors) {
              await ask(
                "Some edge functions failed to deploy. Please check the results.",
                {
                  title: "Deployment Warning",
                  kind: "warning",
                },
              );
            } else {
              await ask("Schema changes pushed successfully", {
                title: "Success",
                kind: "info",
              });
              loadDiff();
              onSuccess();
            }
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
          <FileDiff
            size={16}
            strokeWidth={1}
            className="text-muted-foreground"
          />
          Project Diff
        </h2>
        <div className="flex items-center gap-0">
          <Button
            variant="ghost"
            size="icon"
            onClick={loadDiff}
            title="Refresh diff"
            disabled={isLoading || isPushing}
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
            disabled={isPushing}
          >
            <X size={16} strokeWidth={1} />
          </Button>
        </div>
      </div>

      <div className="flex-1 overflow-auto p-0 bg-muted/25">
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
          (diff.migration_sql.trim() === "" &&
            !diff.is_destructive &&
            diff.edge_functions.length === 0 &&
            !deploymentResults) ? (
          <div className="flex flex-col items-center justify-center h-full p-4 text-center bg-background">
            <p>No changes detected</p>
            <p className="text-sm mt-1 text-muted-foreground">
              Local schema matches remote
            </p>
          </div>
        ) : (
          <div className="flex flex-col h-full">
            {diff.edge_functions.length > 0 && !deploymentResults && (
              <div className="p-4 border-b shrink-0">
                <h3 className="text-xs text-muted-foreground uppercase tracking-wider mb-2 font-mono">
                  Edge Functions ({diff.edge_functions.length})
                </h3>
                <div className="flex items-center gap-2">
                  {diff.edge_functions.map((func) => (
                    <div
                      key={func.slug}
                      className="flex items-center gap-2 text-sm rounded-full py-2 px-3 bg-muted"
                    >
                      <div className="w-1.5 h-1.5 rounded-full bg-yellow-500 shrink-0" />
                      <span className="font-mono text-xs">{func.name}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}
            <div className="flex-1 overflow-auto">
              <h3 className="text-xs text-muted-foreground uppercase tracking-wider mb-2 font-mono p-4 pb-0">
                Schema
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
                // Only show "No schema changes" if we haven't just deployed (and thus maybe cleared it)
                // Actually, if we just deployed (deploymentResults exists), we might still want to see the old SQL or nothing.
                // But typically loadDiff is called on success so this would reset.
                // If invalid deployment, diff is still there.
                !(diff.edge_functions.length > 0) && (
                  <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
                    No schema changes
                  </div>
                )
              )}
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

      {deploymentResults && deploymentResults.length > 0 && (
        <div className="bg-background border-t max-h-60 overflow-auto">
          <div className="p-3 border-b bg-muted/30 flex items-center justify-between sticky top-0 backdrop-blur-sm z-10">
            <h3 className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
              Deployment Results
            </h3>
            {deploymentResults.some((r) => r.status === "error") && (
              <Button
                variant="ghost"
                size="icon"
                className="h-6 w-6"
                onClick={() => {
                  const errorText = deploymentResults
                    .filter((r) => r.status === "error" && r.error)
                    .map((r) => `Function: ${r.name}\nError: ${r.error}`)
                    .join("\n\n");
                  navigator.clipboard.writeText(errorText);
                }}
                title="Copy all errors"
              >
                <Copy size={12} className="text-muted-foreground" />
              </Button>
            )}
          </div>
          <div>
            {deploymentResults.map((result) => (
              <div
                key={result.name}
                className={`p-3 border-b last:border-0 text-sm ${result.status === "error" ? "bg-red-500/5" : ""}`}
              >
                <div className="flex items-center justify-between mb-1">
                  <div className="flex items-center gap-2">
                    {result.status === "success" ? (
                      <CheckCircle size={14} className="text-green-500" />
                    ) : (
                      <AlertTriangle size={14} className="text-red-500" />
                    )}
                    <span className="font-medium">{result.name}</span>
                  </div>
                  {result.version && (
                    <span className="text-xs text-muted-foreground">
                      v{result.version}
                    </span>
                  )}
                </div>
                {result.error && (
                  <pre className="mt-1 text-xs text-red-500 overflow-x-auto whitespace-pre-wrap p-2 bg-red-500/5 rounded border border-red-500/10">
                    {result.error}
                  </pre>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="p-4 border-t shrink-0 bg-background">
        <Button
          className="w-full gap-2"
          onClick={handlePush}
          disabled={
            isLoading ||
            isPushing ||
            !diff ||
            (diff.migration_sql.trim() === "" &&
              !diff.is_destructive &&
              diff.edge_functions.length === 0)
          }
          variant={diff?.is_destructive ? "destructive" : "default"}
        >
          {isPushing ? (
            <>
              <RefreshCw size={16} className="animate-spin" />
              Pushing...
            </>
          ) : (
            <>
              <CloudUpload size={16} />
              {diff?.is_destructive ? "Force Push Changes" : "Push Changes"}
            </>
          )}
        </Button>
      </div>
    </div>
  );
}
