import { useEffect, useRef, useState } from "react";
import * as api from "../api";
import type { Project, SupabaseLogEntry } from "../types";
import "./LogsViewer.css";

export function LogsViewer() {
  const [logs, setLogs] = useState<SupabaseLogEntry[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string>("");
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const logsEndRef = useRef<HTMLDivElement>(null);
  const hasFetchedRef = useRef<boolean>(false);

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
  }, [selectedProjectId]);

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
      // Fetch both Postgres and Edge Function logs
      const [pgLogs, efLogs] = await Promise.all([
        api.getPostgresLogs(projectId, 60 * 24), // Last 24h
        api.getEdgeFunctionLogs(projectId, undefined, 60 * 24),
      ]);

      const normalizedPgLogs = (Array.isArray(pgLogs) ? pgLogs : []).map(
        (log: any) => ({
          id: log.id,
          timestamp: log.timestamp,
          event_message: log.event_message || log.query,
          metadata: {
            user_name: log.user_name,
            error_severity: log.error_severity,
            query: log.query,
          },
          request: null,
          source: "postgres" as const,
          error_severity: log.error_severity,
        })
      );

      const normalizedEfLogs = (Array.isArray(efLogs) ? efLogs : []).map(
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

  const getLogIcon = (log: SupabaseLogEntry) => {
    if (log.error_severity && log.error_severity !== "LOG") return "✕";
    if (log.status && log.status >= 400) return "✕";
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

  return (
    <div className="logs-viewer">
      <div className="logs-header">
        <div className="project-selector">
          <label>Project:</label>
          <select
            value={selectedProjectId}
            onChange={(e) => setSelectedProjectId(e.target.value)}
            disabled={projects.length === 0}
          >
            {projects.length === 0 ? (
              <option value="">No projects found</option>
            ) : (
              projects.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))
            )}
          </select>
        </div>
        <div className="logs-actions">
          <button
            className="refresh-btn"
            onClick={() => selectedProjectId && loadLogs(selectedProjectId)}
          >
            Refresh
          </button>
        </div>
      </div>

      {isLoading ? (
        <div className="loading">Loading logs...</div>
      ) : error ? (
        <div className="empty-state">
          <p className="error-text">{error}</p>
        </div>
      ) : logs.length === 0 ? (
        <div className="empty-state">
          <p>No logs found for this project</p>
          <p className="hint">Logs from Supabase will appear here</p>
        </div>
      ) : (
        <div className="logs-list">
          {logs.map((log) => (
            <div key={log.id} className={`log-entry ${getLogClass(log)}`}>
              <span className="log-icon">{getLogIcon(log)}</span>
              <div className="log-content">
                <div className="log-header">
                  <span className="log-source">{log.source}</span>
                  <span className="log-time">{formatTime(log.timestamp)}</span>
                  {log.status && (
                    <span className={`log-status s-${log.status}`}>
                      {log.status}
                    </span>
                  )}
                </div>
                <div className="log-message">{log.event_message}</div>
                {(log.metadata?.error || log.metadata?.function_id) && (
                  <div className="log-details">
                    {JSON.stringify(log.metadata, null, 2)}
                  </div>
                )}
              </div>
            </div>
          ))}
          <div ref={logsEndRef} />
        </div>
      )}
    </div>
  );
}
