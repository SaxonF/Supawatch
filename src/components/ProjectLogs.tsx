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
import type { SupabaseLogEntry } from "../types";
import { Button } from "./ui/button";
import { HoverCard, HoverCardContent, HoverCardTrigger } from "./ui/hover-card";

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

interface ProjectLogsProps {
  projectId: string;
  expanded: boolean;
  onToggle: () => void;
}

export function ProjectLogs({
  projectId,
  expanded,
  onToggle,
}: ProjectLogsProps) {
  const [logs, setLogs] = useState<SupabaseLogEntry[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const logsEndRef = useRef<HTMLDivElement>(null);
  const hasFetchedRef = useRef<boolean>(false);
  const [showErrorsOnly, setShowErrorsOnly] = useState(false);

  useEffect(() => {
    if (projectId) {
      hasFetchedRef.current = false;
      loadLogs(projectId);
    } else {
      setLogs([]);
    }
  }, [projectId]); // Removed showErrorsOnly from dependency to prevent auto-reload on filter change

  const loadLogs = async (projectId: string) => {
    // Keep loading state mainly for the initial load or explicit refreshes
    if (!hasFetchedRef.current) setIsLoading(true);

    try {
      // Always fetch all logs initially so we can filter client-side or toggle quickly
      // Note: If you want to fetch ONLY errors when filtered, you can keep the server-side filter.
      // For now, let's keep the query fetching everything for the "dots" view unless valid reason not to.
      // But the original code re-fetched when showErrorsOnly changed.
      // Let's stick to the user request: "In collapsed state we just show a series of dots... red if its an error".
      // This implies we need ALL logs to show dots, some red, some not.
      // So ensuring we have all relevant recent logs is good.

      let sql = `select identifier, postgres_logs.timestamp, id, event_message, parsed.error_severity, parsed.detail, parsed.hint
from postgres_logs
cross join unnest(metadata) as m
cross join unnest(m.parsed) as parsed`;

      // If we want to support the filter in expanded mode, we can do client side filtering
      // OR server side. But for the collapsed view "dots", we likely want to see the stream.
      // The prompt says "one dot for each log item, and red if its an error".
      // So we should probably fetch everything.

      sql += ` order by timestamp desc limit 100`;

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

      const allLogs = [...normalizedPgLogs, ...normalizedEfLogs].sort(
        (a, b) =>
          new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
      );

      setLogs(allLogs);
      setError(null);
      hasFetchedRef.current = true;
    } catch (err: any) {
      console.error("Failed to load logs:", err);
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

  const filteredLogs = showErrorsOnly
    ? logs.filter((log) => {
        return (
          log.error_severity === "ERROR" ||
          log.error_severity === "FATAL" ||
          log.error_severity === "PANIC" ||
          (log.status && log.status >= 400)
        );
      })
    : logs;

  const isError = (log: SupabaseLogEntry) => {
    return (
      log.error_severity === "ERROR" ||
      log.error_severity === "FATAL" ||
      log.error_severity === "PANIC" ||
      (log.status && log.status >= 400)
    );
  };

  if (!expanded) {
    return (
      <div
        className="flex flex-col h-full border-l bg-background w-[50px] items-center py-3 gap-3 cursor-pointer hover:bg-muted/30 transition-colors"
        onClick={onToggle}
      >
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8 text-muted-foreground hover:text-primary mb-2"
          onClick={(e) => {
            e.stopPropagation();
            projectId && loadLogs(projectId);
          }}
          title="Refresh logs"
        >
          <RefreshCcw size={14} className={isLoading ? "animate-spin" : ""} />
        </Button>

        <div className="flex-1 w-full flex flex-col items-center gap-1.5 overflow-hidden mask-gradient-b pb-4">
          {logs.slice(0, 30).map((log) => (
            <HoverCard key={log.id} openDelay={0} closeDelay={0}>
              <HoverCardTrigger asChild>
                <div
                  className={`w-2 h-2 rounded-full shrink-0 transition-all hover:scale-150 ${
                    isError(log) ? "bg-red-500" : "bg-muted-foreground/30"
                  }`}
                  onClick={(e) => e.stopPropagation()} // Preventing sidebar expansion on clicking the dot itself if user just wants to see hover, but user said "clicking sidebar opens it", so maybe we allow it?
                  // Review says: "Clicking the sidebar in collapsed state opens it".
                  // If I stop propagation here, clicking the dot won't open it.
                  // Let's NOT stop propagation on the dot so clicking it also opens the sidebar, which feels natural.
                  // But wait, if they are clicking the dot they might expect to see more details or interact with the hover card?
                  // Hover card is hover-based.
                  // So clicking the dot effectively is "clicking the sidebar".
                />
              </HoverCardTrigger>
              <HoverCardContent
                side="left"
                align="start"
                className="w-[300px] z-50 p-3"
              >
                <div className="flex flex-col gap-2">
                  <div className="flex items-start justify-between gap-2">
                    <span
                      className={`text-xs font-mono font-medium ${
                        isError(log) ? "text-red-500" : "text-muted-foreground"
                      }`}
                    >
                      {log.error_severity || "LOG"}
                    </span>
                    <span className="text-[10px] text-muted-foreground whitespace-nowrap">
                      {new Date(log.timestamp).toLocaleTimeString()}
                    </span>
                  </div>
                  <p className="text-xs text-foreground/90 break-words font-mono line-clamp-4">
                    {log.event_message}
                  </p>
                </div>
              </HoverCardContent>
            </HoverCard>
          ))}
          {logs.length > 30 && (
            <div className="w-1 h-1 rounded-full bg-muted-foreground/20 mt-1" />
          )}
        </div>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full overflow-hidden border-l w-[400px] bg-background">
      <div className="flex items-center justify-between px-4 py-3 border-b shrink-0">
        <h3 className="font-medium text-sm">Logs</h3>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-muted-foreground hover:text-primary"
            onClick={() => setShowErrorsOnly(!showErrorsOnly)}
            title={showErrorsOnly ? "Showing errors only" : "Showing all logs"}
          >
            <Filter
              size={14}
              className={showErrorsOnly ? "text-destructive font-bold" : ""}
            />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-muted-foreground hover:text-primary"
            onClick={() => projectId && loadLogs(projectId)}
            title="Refresh logs"
          >
            <RefreshCcw size={14} className={isLoading ? "animate-spin" : ""} />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-muted-foreground hover:text-primary"
            onClick={onToggle}
            title="Minimize sidebar"
          >
            <ChevronRight size={16} />
          </Button>
        </div>
      </div>
      <div className="flex-1 overflow-auto p-4 custom-scrollbar">
        {isLoading && logs.length === 0 ? (
          <div className="flex items-center justify-center h-full text-muted-foreground">
            <RefreshCcw size={16} className="animate-spin mr-2" />
            Loading...
          </div>
        ) : error ? (
          <div className="flex items-center justify-center h-full">
            <p className="text-destructive text-sm text-center px-4">{error}</p>
          </div>
        ) : filteredLogs.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
            <p className="text-sm">No logs found</p>
          </div>
        ) : (
          <div className="space-y-2">
            {filteredLogs.map((log) => (
              <LogEntryItem key={log.id} log={log} />
            ))}
            <div ref={logsEndRef} />
          </div>
        )}
      </div>
    </div>
  );
}
