import {
  ChevronDown,
  ChevronRight,
  Code,
  Database,
  Filter,
  RefreshCcw,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { vscDarkPlus } from "react-syntax-highlighter/dist/esm/styles/prism";
import * as api from "../api";
import type { Project, SupabaseLogEntry } from "../types";
import { Button } from "./ui/button";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";

function LogEntryItem({ log }: { log: SupabaseLogEntry }) {
  const [showMetadata, setShowMetadata] = useState(false);

  const getLogIcon = (log: SupabaseLogEntry) => {
    if (log.source === "postgres")
      return (
        <Database
          strokeWidth={1.5}
          size={16}
          className="text-muted-foreground/50"
        />
      );
    if (log.source === "edge_function")
      return (
        <Code
          strokeWidth={1.5}
          size={16}
          className="text-muted-foreground/50"
        />
      );
    return "•";
  };

  const getLogClass = (log: SupabaseLogEntry) => {
    if (log.error_severity && log.error_severity !== "LOG") return "error";
    if (log.status && log.status >= 400) return "error";
    if (log.source === "edge_function") return "function";
    return "postgres";
  };

  const formatTime = (timestamp: string) => {
    const date = new Date(timestamp);
    return date.toLocaleTimeString("en-US", {
      hour: "2-digit",
      minute: "2-digit",
      second: "2-digit",
      hour12: false,
    });
  };

  const hasMetadata =
    log.metadata?.error ||
    log.metadata?.function_id ||
    log.metadata?.detail ||
    log.metadata?.hint ||
    log.metadata?.identifier;

  return (
    <div className={`bg-muted ${getLogClass(log)}`}>
      <div className="flex items-center gap-3 p-3 shadow-sm rounded-lg">
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => hasMetadata && setShowMetadata(!showMetadata)}
          className={!hasMetadata ? "opacity-0 pointer-events-none" : ""}
        >
          {showMetadata ? (
            <ChevronDown size={14} className="text-muted-foreground" />
          ) : (
            <ChevronRight size={14} className="text-muted-foreground" />
          )}
        </Button>
        <div className="flex-1 whitespace-pre-wrap break-all flex flex-col gap-1">
          <div className="flex items-baseline gap-2">
            <span className="text-foreground">{log.event_message}</span>
          </div>
        </div>
        {(log.error_severity || log.status) && (
          <span
            className={`text-xs px-2 py-0.5 rounded-full ${
              log.error_severity === "ERROR" ||
              log.error_severity === "FATAL" ||
              log.error_severity === "PANIC" ||
              (log.status && log.status >= 400)
                ? "bg-red-500/10 text-red-500"
                : "bg-blue-500/10 text-blue-500"
            }`}
          >
            {log.error_severity || log.status}
          </span>
        )}
        <span className="text-mono text-xs text-muted-foreground whitespace-nowrap">
          {formatTime(log.timestamp)}
        </span>
        <span className="log-icon">{getLogIcon(log)}</span>
      </div>
      {showMetadata && hasMetadata && (
        <div className="p-0 text-xs bg-background/50">
          <SyntaxHighlighter
            language="json"
            style={vscDarkPlus}
            customStyle={{
              margin: 0,
              padding: "1rem",
              background: "transparent",
              fontSize: "11px",
            }}
            codeTagProps={{
              style: {
                fontSize: "inherit",
              },
            }}
            wrapLongLines={true}
          >
            {JSON.stringify(log.metadata, null, 2)}
          </SyntaxHighlighter>
        </div>
      )}
    </div>
  );
}

export function LogsViewer() {
  const [logs, setLogs] = useState<SupabaseLogEntry[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string>("");
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const logsEndRef = useRef<HTMLDivElement>(null);
  const hasFetchedRef = useRef<boolean>(false);

  const [showErrorsOnly, setShowErrorsOnly] = useState(true);

  useEffect(() => {
    loadProjects();
  }, []);

  useEffect(() => {
    if (selectedProjectId) {
      // Reset state for new project
      hasFetchedRef.current = false;
      loadLogs(selectedProjectId);
    } else {
      setLogs([]);
    }
  }, [selectedProjectId, showErrorsOnly]);

  const loadProjects = async () => {
    try {
      const data = await api.getProjects();
      const validProjects = data.filter((p) => p.supabase_project_id);
      setProjects(validProjects);
      if (validProjects.length > 0) {
        setSelectedProjectId(validProjects[0].id);
      }
    } catch (err) {
      console.error("Failed to load projects:", err);
    }
  };

  const loadLogs = async (projectId: string) => {
    // Only show loading indicator on first fetch for this project
    if (!hasFetchedRef.current) setIsLoading(true);

    try {
      let sql = `select identifier, postgres_logs.timestamp, id, event_message, parsed.error_severity, parsed.detail, parsed.hint 
from postgres_logs
cross join unnest(metadata) as m
cross join unnest(m.parsed) as parsed`;

      if (showErrorsOnly) {
        sql += ` where parsed.error_severity in ('ERROR', 'FATAL', 'PANIC')`;
      }

      sql += ` order by timestamp desc limit 100`;

      // Fetch both Postgres and Edge Function logs
      // Note: Edge function logs filtering is done client-side as we use a dedicated endpoint
      const [pgLogsResult, efLogs] = await Promise.all([
        api.querySupabaseLogs(projectId, sql),
        api.getEdgeFunctionLogs(projectId, undefined, 60 * 24),
      ]);

      const pgLogs = Array.isArray(pgLogsResult) ? pgLogsResult : [];

      const normalizedPgLogs = pgLogs.map((log: any) => ({
        id: log.id,
        timestamp: log.timestamp,
        event_message: log.event_message,
        metadata: {
          identifier: log.identifier,
          error_severity: log.error_severity,
          detail: log.detail,
          hint: log.hint,
        },
        request: null,
        source: "postgres" as const,
        error_severity: log.error_severity,
      }));

      let normalizedEfLogs = (Array.isArray(efLogs) ? efLogs : []).map(
        (log: any) => ({
          id: log.id,
          timestamp: log.timestamp,
          event_message: log.event_message,
          metadata: {
            function_id: log.function_id,
            execution_time_ms: log.execution_time_ms,
            deployment_id: log.deployment_id,
            version: log.version,
            method: log.method,
            url: log.url,
          },
          request: {
            method: log.method,
            url: log.url,
          },
          source: "edge_function" as const,
          status: log.status_code,
        })
      );

      // Client-side filtering for Edge Function logs
      if (showErrorsOnly) {
        normalizedEfLogs = normalizedEfLogs
          .filter(
            (log: any) =>
              (log.status && log.status >= 400) ||
              log.source === "edge_function" // Keep all edge function logs? Or just errors?
            // User said "I only want to show error logs by default."
            // Usually edge functions with status >= 400 are errors.
            // Let's filter strictly by status >= 400 for Edge Logs if filtering is on.
          )
          .filter((log: any) => log.status >= 400);
      }

      const allLogs = [...normalizedPgLogs, ...normalizedEfLogs].sort(
        (a, b) =>
          new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
      );

      setLogs(allLogs);
      setError(null);
      hasFetchedRef.current = true;
    } catch (err: any) {
      // If the custom query fails (e.g. schema mismatch), we might want to fallback or just show error.
      // Since the user provided the query, we assume it works.
      console.error("Failed to load logs:", err);
      // Show error only if it's the first fetch attempt
      if (!hasFetchedRef.current) {
        setError(
          typeof err === "string"
            ? err
            : err.message || "Failed to load logs from Supabase"
        );
      }
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div className="flex items-center gap-4 justify-between p-5 pb-0 shrink-0">
        <Select
          value={selectedProjectId}
          onValueChange={setSelectedProjectId}
          disabled={projects.length === 0}
        >
          <SelectTrigger className="flex-1 h-10">
            <SelectValue placeholder="Select project" />
          </SelectTrigger>
          <SelectContent>
            {projects.length === 0 ? (
              <SelectItem value="none" disabled>
                No projects found
              </SelectItem>
            ) : (
              projects.map((p) => (
                <SelectItem key={p.id} value={p.id}>
                  {p.name}
                </SelectItem>
              ))
            )}
          </SelectContent>
        </Select>
        <div className="logs-actions flex items-center gap-2">
          <Button
            variant={"outline"}
            size="icon"
            onClick={() => setShowErrorsOnly(!showErrorsOnly)}
            title={showErrorsOnly ? "Showing errors only" : "Showing all logs"}
          >
            <Filter
              size={16}
              strokeWidth={1}
              className={showErrorsOnly ? "text-destructive" : ""}
            />
          </Button>
          <Button
            variant="outline"
            size="icon"
            onClick={() => selectedProjectId && loadLogs(selectedProjectId)}
          >
            <RefreshCcw size={16} strokeWidth={1} />
          </Button>
        </div>
      </div>
      <div className="flex-1 overflow-auto p-5">
        {isLoading ? (
          <div className="loading">Loading logs...</div>
        ) : error ? (
          <div className="text-center p-16">
            <p className="error-text">{error}</p>
          </div>
        ) : logs.length === 0 ? (
          <div className="text-center p-16">
            <p>No logs found for this project</p>
            <p className="hint">Logs from Supabase will appear here</p>
          </div>
        ) : (
          <div className="rounded-xl overflow-hidden space-y-1">
            {logs.map((log) => (
              <LogEntryItem key={log.id} log={log} />
            ))}
            <div ref={logsEndRef} />
          </div>
        )}
      </div>
    </div>
  );
}
