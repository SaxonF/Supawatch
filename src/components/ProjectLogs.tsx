import { cn } from "@/lib/utils";
import {
  ChevronRight,
  Code,
  Copy,
  Database,
  FileText,
  Filter,
  Globe,
  RefreshCcw,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Prism as SyntaxHighlighter } from "react-syntax-highlighter";
import { vscDarkPlus } from "react-syntax-highlighter/dist/esm/styles/prism";
import * as api from "../api";
import type { SupabaseLogEntry } from "../types";
import { Button } from "./ui/button";
import { HoverCard, HoverCardContent, HoverCardTrigger } from "./ui/hover-card";

const getLogIcon = (log: SupabaseLogEntry) => {
  if (log.source === "postgres") return <Database strokeWidth={1} size={14} />;
  if (log.source === "edge_function") return <Code strokeWidth={1} size={14} />;
  if (log.source === "edge_function_log")
    return <FileText strokeWidth={1} size={14} />;
  if (log.source === "api_gateway") return <Globe strokeWidth={1} size={14} />;
  return <span>•</span>;
};

interface LogEntryItemProps {
  log: SupabaseLogEntry;
  isSelected: boolean;
  isNextSelected: boolean;
  onSelect: (e: React.MouseEvent) => void;
}

function LogEntryItem({
  log,
  isSelected,
  isNextSelected,
  onSelect,
}: LogEntryItemProps) {
  const [showMetadata, setShowMetadata] = useState(false);
  const mouseDownPos = useRef<{ x: number; y: number } | null>(null);

  const getLogClass = (log: SupabaseLogEntry) => {
    if (
      log.error_severity &&
      !["LOG", "INFO", "DEBUG"].includes(log.error_severity)
    )
      return "error";
    if (log.status && log.status >= 400) return "error";
    if (log.source === "edge_function") return "function";
    if (log.source === "edge_function_log") return "function";
    if (log.source === "api_gateway") return "api";
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

  const handleMouseDown = (e: React.MouseEvent) => {
    mouseDownPos.current = { x: e.clientX, y: e.clientY };
    // Prevent text selection when shift-clicking for range selection
    if (e.shiftKey) {
      e.preventDefault();
    }
  };

  const handleMouseUp = (e: React.MouseEvent) => {
    // Only trigger selection if this was a clean click (not a drag to select text)
    if (mouseDownPos.current) {
      const dx = Math.abs(e.clientX - mouseDownPos.current.x);
      const dy = Math.abs(e.clientY - mouseDownPos.current.y);
      // Allow small movement (5px threshold for accidental micro-movements)
      if (dx < 5 && dy < 5) {
        onSelect(e);
      }
    }
    mouseDownPos.current = null;
  };

  return (
    <div
      onMouseDown={handleMouseDown}
      onMouseUp={handleMouseUp}
      className={`${getLogClass(log)} cursor-pointer group`}
    >
      <div
        className={cn(
          "flex flex-col gap-3 p-4 group  border-b border-border/75",
          showMetadata && "bg-muted/25",
          !isSelected && "hover:bg-muted/25",
          isSelected && "bg-primary/10 select-none",
          isSelected && isNextSelected && "border-b-primary/10"
        )}
      >
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            {(log.error_severity || log.status || log.level) && (
              <span
                className={`text-xs px-2 py-1 rounded font-mono uppercase flex items-center gap-2 ${
                  log.error_severity === "ERROR" ||
                  log.error_severity === "FATAL" ||
                  log.error_severity === "PANIC" ||
                  log.level === "error" ||
                  (log.status && log.status >= 400)
                    ? "bg-red-500/10 text-red-500"
                    : "bg-muted text-muted-foreground"
                }`}
              >
                {getLogIcon(log)}
                {log.error_severity || log.status || log.level}
              </span>
            )}
            {hasMetadata && (
              <Button
                variant={"outline"}
                onClick={(e) => {
                  e.stopPropagation();
                  setShowMetadata(!showMetadata);
                }}
                size={"icon-sm"}
                onMouseDown={(e) => e.stopPropagation()}
                onMouseUp={(e) => e.stopPropagation()}
                title={showMetadata ? "Collapse metadata" : "Expand metadata"}
                className="h-6 w-6 flex justify-center items-center text-center p-0 hidden group-hover:flex"
              >
                <ChevronRight
                  size={14}
                  strokeWidth={1}
                  className={cn(
                    "transition-transform m-0",
                    showMetadata && "rotate-90"
                  )}
                />
              </Button>
            )}
          </div>
          <span className="font-mono text-sm text-muted-foreground/50 whitespace-nowrap">
            {formatTime(log.timestamp)}
          </span>
        </div>
        <div className="flex-1 whitespace-pre-wrap break-all flex flex-col gap-1">
          <div className="flex items-baseline gap-2">
            <span className="text-muted-foreground group-hover:text-foreground">
              {log.event_message}
            </span>
          </div>
        </div>
      </div>
      {showMetadata && hasMetadata && (
        <div className="p-0 text-xs bg-muted/25">
          <SyntaxHighlighter
            language="json"
            style={vscDarkPlus}
            customStyle={{
              margin: 0,
              padding: "1.5rem",
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
  const [selectedLogIds, setSelectedLogIds] = useState<Set<string>>(new Set());
  const [lastSelectedIndex, setLastSelectedIndex] = useState<number | null>(
    null
  );

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

      const [pgLogsResult, efInvocationsResult, efLogsResult, apiLogsResult] =
        await Promise.all([
          api.querySupabaseLogs(projectId, sql),
          api.querySupabaseLogs(
            projectId,
            `select id, function_edge_logs.timestamp, event_message, response.status_code, request.method, m.function_id, m.execution_time_ms, m.deployment_id, m.version from function_edge_logs
  cross join unnest(metadata) as m
  cross join unnest(m.response) as response
  cross join unnest(m.request) as request
  order by timestamp desc
  limit 100`
          ),
          api.querySupabaseLogs(
            projectId,
            `select id, function_logs.timestamp, event_message, metadata.event_type, metadata.function_id, metadata.level from function_logs
  cross join unnest(metadata) as metadata
  order by timestamp desc
  limit 100`
          ),
          api.querySupabaseLogs(
            projectId,
            `select id, identifier, timestamp, event_message, request.method, request.path, request.search, response.status_code
  from edge_logs
  cross join unnest(metadata) as m
  cross join unnest(m.request) as request
  cross join unnest(m.response) as response
  
  order by timestamp desc
  limit 100`
          ),
        ]);

      const pgLogs = Array.isArray(pgLogsResult) ? pgLogsResult : [];
      const apiLogs = Array.isArray(apiLogsResult) ? apiLogsResult : [];

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

      const normalizedEfInvocations = (
        Array.isArray(efInvocationsResult) ? efInvocationsResult : []
      ).map((log: any) => ({
        id: log.id,
        timestamp: log.timestamp,
        event_message: log.event_message,
        metadata: {
          function_id: log.function_id,
          execution_time_ms: log.execution_time_ms,
          deployment_id: log.deployment_id,
          version: log.version,
          method: log.method,
        },
        request: {
          method: log.method,
        },
        source: "edge_function" as const,
        status: log.status_code,
      }));

      const normalizedEfLogs = (
        Array.isArray(efLogsResult) ? efLogsResult : []
      ).map((log: any) => ({
        id: log.id,
        timestamp: log.timestamp,
        event_message: log.event_message,
        metadata: {
          event_type: log.event_type,
          function_id: log.function_id,
          level: log.level,
        },
        request: null,
        source: "edge_function_log" as const,
        error_severity: log.level ? log.level.toUpperCase() : "LOG",
      }));

      const normalizedApiLogs = apiLogs.map((log: any) => ({
        id: log.id,
        timestamp: log.timestamp,
        event_message: `${log.method} ${log.path}${log.search || ""}`,
        metadata: {
          identifier: log.identifier,
          method: log.method,
          path: log.path,
          search: log.search,
          status_code: log.status_code,
          original_message: log.event_message,
        },
        request: {
          method: log.method,
          url: log.path + (log.search || ""),
        },
        source: "api_gateway" as const,
        status: log.status_code,
      }));

      const allLogs = [
        ...normalizedPgLogs,
        ...normalizedEfInvocations,
        ...normalizedEfLogs,
        ...normalizedApiLogs,
      ].sort(
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

  const handleLogSelect = (
    logId: string,
    index: number,
    e: React.MouseEvent
  ) => {
    if (e.shiftKey && lastSelectedIndex !== null) {
      // Shift+click: select range
      const start = Math.min(lastSelectedIndex, index);
      const end = Math.max(lastSelectedIndex, index);
      const newSelectedIds = new Set<string>();
      for (let i = start; i <= end; i++) {
        newSelectedIds.add(filteredLogs[i].id);
      }
      setSelectedLogIds(newSelectedIds);
    } else {
      if (selectedLogIds.has(logId)) {
        // If already selected, deselect it and any others
        setSelectedLogIds(new Set());
        setLastSelectedIndex(null);
      } else {
        // Normal click: select only this one (clear others)
        const newSelectedIds = new Set<string>();
        newSelectedIds.add(logId);
        setSelectedLogIds(newSelectedIds);
        setLastSelectedIndex(index);
      }
    }
  };

  const copySelectedLogs = async () => {
    const selectedLogs = filteredLogs.filter((log) =>
      selectedLogIds.has(log.id)
    );
    const logText = selectedLogs
      .map((log) => {
        const timestamp = new Date(log.timestamp).toISOString();
        const severity = log.error_severity || log.status || log.level || "";
        const source = log.source || "unknown";
        let text = `[${timestamp}] [${source}] ${
          severity ? `[${severity}] ` : ""
        }${log.event_message}`;
        if (log.metadata && Object.keys(log.metadata).length > 0) {
          text += `\nMetadata: ${JSON.stringify(log.metadata, null, 2)}`;
        }
        return text;
      })
      .join("\n\n---\n\n");

    await navigator.clipboard.writeText(logText);
  };

  const clearSelection = () => {
    setSelectedLogIds(new Set());
    setLastSelectedIndex(null);
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
          <RefreshCcw
            size={14}
            strokeWidth={1}
            className={isLoading ? "animate-spin" : ""}
          />
        </Button>

        <div className="flex-1 w-full flex flex-col items-center gap-1.5 overflow-hidden mask-gradient-b pb-4">
          {logs.slice(0, 30).map((log) => (
            <HoverCard key={log.id} openDelay={0} closeDelay={0}>
              <HoverCardTrigger asChild>
                <div
                  className={`w-2 h-2 rounded-full shrink-0 transition-all ${
                    isError(log)
                      ? "bg-destructive"
                      : "bg-muted-foreground/30 hover:bg-foreground"
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
                    {(log.error_severity || log.status) && (
                      <span
                        className={`text-xs px-2 py-1 rounded font-mono uppercase flex items-center gap-2 ${
                          log.error_severity === "ERROR" ||
                          log.error_severity === "FATAL" ||
                          log.error_severity === "PANIC" ||
                          (log.status && log.status >= 400)
                            ? "bg-red-500/10 text-red-500"
                            : "bg-muted text-muted-foreground"
                        }`}
                      >
                        {getLogIcon(log)}
                        {log.error_severity || log.status}
                      </span>
                    )}
                    <span className="text-[10px] text-muted-foreground whitespace-nowrap">
                      {new Date(log.timestamp).toLocaleTimeString()}
                    </span>
                  </div>
                  <p className="text-sm text-foreground/90 break-words font-mono line-clamp-4">
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
        <h3 className="font-medium">Logs</h3>
        <div className="flex items-center gap-1">
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-muted-foreground hover:text-primary"
            onClick={() => setShowErrorsOnly(!showErrorsOnly)}
            title={showErrorsOnly ? "Showing errors only" : "Showing all logs"}
          >
            <Filter
              size={16}
              strokeWidth={1}
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
            <RefreshCcw
              size={16}
              strokeWidth={1}
              className={isLoading ? "animate-spin" : ""}
            />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-muted-foreground hover:text-primary"
            onClick={onToggle}
            title="Minimize sidebar"
          >
            <ChevronRight size={16} strokeWidth={1} />
          </Button>
        </div>
      </div>
      <div className="flex-1 overflow-auto custom-scrollbar">
        {isLoading && logs.length === 0 ? (
          <div className="flex items-center justify-center h-full text-muted-foreground">
            <RefreshCcw
              size={16}
              strokeWidth={1}
              className="animate-spin mr-2"
            />
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
          <div className="">
            {filteredLogs.map((log, index) => {
              const isSelected = selectedLogIds.has(log.id);
              const nextLog = filteredLogs[index + 1];
              const isNextSelected = nextLog
                ? selectedLogIds.has(nextLog.id)
                : false;
              return (
                <LogEntryItem
                  key={log.id}
                  log={log}
                  isSelected={isSelected}
                  isNextSelected={isNextSelected}
                  onSelect={(e) => handleLogSelect(log.id, index, e)}
                />
              );
            })}
            <div ref={logsEndRef} />
          </div>
        )}
      </div>
      {selectedLogIds.size > 0 && (
        <div className="shrink-0 px-4 py-3 border-t bg-muted/50 flex items-center justify-between">
          <span className="text-muted-foreground">
            {selectedLogIds.size} log{selectedLogIds.size !== 1 ? "s" : ""}{" "}
            selected
          </span>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={clearSelection}>
              Clear
            </Button>
            <Button size="sm" onClick={copySelectedLogs}>
              <Copy size={14} strokeWidth={1} />
              Copy
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
