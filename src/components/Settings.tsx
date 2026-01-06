import { useEffect, useState } from "react";
import * as api from "../api";
import "./Settings.css";

export function Settings() {
  const [token, setToken] = useState("");
  const [hasToken, setHasToken] = useState(false);
  const [isValidating, setIsValidating] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  useEffect(() => {
    checkToken();
  }, []);

  const checkToken = async () => {
    const has = await api.hasAccessToken();
    setHasToken(has);
  };

  const handleSave = async () => {
    if (!token.trim()) {
      setError("Please enter an access token");
      return;
    }

    setError(null);
    setSuccess(null);
    setIsSaving(true);

    try {
      await api.setAccessToken(token.trim());
      setIsValidating(true);

      const isValid = await api.validateAccessToken();
      setIsValidating(false);

      if (isValid) {
        setSuccess("Access token saved and validated successfully");
        setHasToken(true);
        setToken("");
      } else {
        await api.clearAccessToken();
        setError("Invalid access token. Please check and try again.");
        setHasToken(false);
      }
    } catch (err) {
      setError(String(err));
    } finally {
      setIsSaving(false);
      setIsValidating(false);
    }
  };

  const handleClear = async () => {
    try {
      await api.clearAccessToken();
      setHasToken(false);
      setSuccess("Access token cleared");
      setError(null);
    } catch (err) {
      setError(String(err));
    }
  };

  return (
    <div className="settings">
      <h3>Settings</h3>

      <div className="settings-section">
        <label>Supabase Personal Access Token</label>
        <p className="hint">
          Generate a token at{" "}
          <a
            href="https://supabase.com/dashboard/account/tokens"
            target="_blank"
            rel="noopener noreferrer"
          >
            supabase.com/dashboard/account/tokens
          </a>
        </p>

        {hasToken ? (
          <div className="token-status">
            <span className="token-saved">Token configured</span>
            <button className="clear-token-btn" onClick={handleClear}>
              Clear Token
            </button>
          </div>
        ) : (
          <div className="token-input-group">
            <input
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              placeholder="sbp_xxxxxxxxxxxxxxxxxxxxxxxx"
              disabled={isSaving}
            />
            <button
              className="save-token-btn"
              onClick={handleSave}
              disabled={isSaving || !token.trim()}
            >
              {isValidating ? "Validating..." : isSaving ? "Saving..." : "Save"}
            </button>
          </div>
        )}

        {error && <div className="error-message">{error}</div>}
        {success && <div className="success-message">{success}</div>}
      </div>

      <div className="settings-section">
        <label>About</label>
        <p className="about-text">
          Supawatch monitors your local Supabase project folders for changes to
          schema files and edge functions, then syncs them to your remote
          Supabase project.
        </p>
      </div>
      <div className="settings-section">
        <label>Audit Logs</label>
        <div className="audit-logs-container">
          <AuditLogs />
        </div>
      </div>
    </div>
  );
}

function AuditLogs() {
  const [logs, setLogs] = useState<import("../types").LogEntry[]>([]);

  useEffect(() => {
    loadLogs();

    // Listen for real-time log updates
    let unlistenFn: (() => void) | undefined;

    import("@tauri-apps/api/event").then(async ({ listen }) => {
      unlistenFn = await listen<import("../types").LogEntry>("log", (event) => {
        setLogs((prev) => [event.payload, ...prev].slice(0, 50));
      });
    });

    return () => {
      if (unlistenFn) unlistenFn();
    };
  }, []);

  const loadLogs = async () => {
    try {
      const data = await api.getLogs(undefined, 50);
      setLogs(data);
    } catch (err) {
      console.error("Failed to load audit logs:", err);
    }
  };

  if (logs.length === 0) {
    return <div className="audit-empty">No system activity recorded</div>;
  }

  return (
    <div className="audit-list">
      {logs.map((log) => (
        <div key={log.id} className={`audit-entry ${log.level}`}>
          <span className="audit-time">
            {new Date(log.timestamp).toLocaleTimeString([], { hour12: false })}
          </span>
          <span className="audit-source">{log.source}</span>
          <span className="audit-message">{log.message}</span>
        </div>
      ))}
    </div>
  );
}
