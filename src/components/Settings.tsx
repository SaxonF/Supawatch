import { useEffect, useState } from "react";
import * as api from "../api";
import { Button } from "./ui/button";

export function Settings() {
  const [token, setToken] = useState("");
  const [hasToken, setHasToken] = useState(false);
  const [isValidating, setIsValidating] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [, setSuccess] = useState<string | null>(null);

  // OpenAI key state
  const [openAiKey, setOpenAiKey] = useState("");
  const [hasOpenAiKey, setHasOpenAiKey] = useState(false);
  const [isSavingOpenAi, setIsSavingOpenAi] = useState(false);
  const [openAiError, setOpenAiError] = useState<string | null>(null);

  useEffect(() => {
    checkToken();
    checkOpenAiKey();
  }, []);

  const checkToken = async () => {
    const has = await api.hasAccessToken();
    setHasToken(has);
  };

  const checkOpenAiKey = async () => {
    const has = await api.hasOpenAiKey();
    setHasOpenAiKey(has);
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

  const handleSaveOpenAiKey = async () => {
    if (!openAiKey.trim()) {
      setOpenAiError("Please enter an OpenAI API key");
      return;
    }

    setOpenAiError(null);
    setIsSavingOpenAi(true);

    try {
      await api.setOpenAiKey(openAiKey.trim());
      setHasOpenAiKey(true);
      setOpenAiKey("");
    } catch (err) {
      setOpenAiError(String(err));
    } finally {
      setIsSavingOpenAi(false);
    }
  };

  const handleClearOpenAiKey = async () => {
    try {
      await api.clearOpenAiKey();
      setHasOpenAiKey(false);
      setOpenAiError(null);
    } catch (err) {
      setOpenAiError(String(err));
    }
  };

  return (
    <div className="space-y-4">
      <p className="text-muted-foreground">
        Supawatch monitors your local Supabase project folders for changes to
        schema files and edge functions, then syncs them to your remote Supabase
        project.
      </p>

      <div>
        <label className="block mb-2">Supabase Personal Access Token</label>
        {hasToken ? (
          <div className="flex items-center gap-2">
            <input
              type="password"
              readOnly
              value={token}
              onChange={(e) => setToken(e.target.value)}
              placeholder="sbp_xxxxxxxxxxxxxxxxxxxxxxxx"
              className="bg-chart-2/25 border border-chart-2 rounded-xl h-12 px-6 block w-full"
              disabled={isSaving}
            />
            <Button
              variant="outline"
              className="h-12 px-6 rounded-xl"
              onClick={handleClear}
            >
              Clear
            </Button>
          </div>
        ) : (
          <div className="w-full flex items-center gap-2">
            <input
              type="password"
              value={token}
              onChange={(e) => setToken(e.target.value)}
              placeholder="sbp_xxxxxxxxxxxxxxxxxxxxxxxx"
              className="bg-input border border-border rounded-xl h-12 px-6 block w-full"
              disabled={isSaving}
            />
            <Button
              className="h-12 px-6 rounded-xl"
              onClick={handleSave}
              disabled={isSaving || !token.trim()}
            >
              {isValidating ? "Validating..." : isSaving ? "Saving..." : "Save"}
            </Button>
          </div>
        )}

        {error && <div className="mt-2 text-destructive text-sm">{error}</div>}
      </div>

      <div>
        <label className="block mb-2">OpenAI API Key</label>
        <p className="text-muted-foreground text-sm mb-2">
          Used for natural language to SQL conversion in the SQL editor.
        </p>
        {hasOpenAiKey ? (
          <div className="flex items-center gap-2">
            <input
              type="password"
              readOnly
              value={openAiKey}
              onChange={(e) => setOpenAiKey(e.target.value)}
              placeholder="sk-xxxxxxxxxxxxxxxxxxxxxxxx"
              className="bg-chart-2/25 border border-chart-2 rounded-xl h-12 px-6 block w-full"
              disabled={isSavingOpenAi}
            />
            <Button
              variant="outline"
              className="h-12 px-6 rounded-xl"
              onClick={handleClearOpenAiKey}
            >
              Clear
            </Button>
          </div>
        ) : (
          <div className="w-full flex items-center gap-2">
            <input
              type="password"
              value={openAiKey}
              onChange={(e) => setOpenAiKey(e.target.value)}
              placeholder="sk-xxxxxxxxxxxxxxxxxxxxxxxx"
              className="bg-input border border-border rounded-xl h-12 px-6 block w-full"
              disabled={isSavingOpenAi}
            />
            <Button
              className="h-12 px-6 rounded-xl"
              onClick={handleSaveOpenAiKey}
              disabled={isSavingOpenAi || !openAiKey.trim()}
            >
              {isSavingOpenAi ? "Saving..." : "Save"}
            </Button>
          </div>
        )}

        {openAiError && (
          <div className="mt-2 text-destructive text-sm">{openAiError}</div>
        )}
      </div>

      <div>
        <label className="block mb-2">Audit Logs</label>
        <div className="border border-border rounded-xl overflow-hidden max-h-48 overflow-auto">
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
    return (
      <div className="text-center p-8 text-muted-foreground">
        No system activity recorded
      </div>
    );
  }

  return (
    <div className="">
      {logs.map((log) => (
        <div
          key={log.id}
          className={`bg-muted p-3 flex items-center gap-4 ${log.level}`}
        >
          <span className="font-mono text-xs text-muted-foreground">
            {new Date(log.timestamp).toLocaleTimeString([], { hour12: false })}
          </span>
          {/* <span className="audit-source">{log.source}</span> */}
          <span className="text-xs font-mono">{log.message}</span>
        </div>
      ))}
    </div>
  );
}
