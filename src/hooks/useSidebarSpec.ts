import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import * as api from "../api";
import type { SidebarSpec } from "../specs/types";

interface AdminConfigChangedPayload {
  project_id: string;
}

/**
 * Hook to manage the sidebar spec for a project.
 * Fetches the spec from admin.json (or returns default) and listens for changes.
 */
export function useSidebarSpec(projectId: string | null) {
  const [sidebarSpec, setSidebarSpec] = useState<SidebarSpec | null>(null);
  const [hasAdminFile, setHasAdminFile] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadSpec = useCallback(async () => {
    if (!projectId) {
      setSidebarSpec(null);
      setHasAdminFile(false);
      setIsLoading(false);
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const [spec, hasFile] = await Promise.all([
        api.getSidebarSpec(projectId),
        api.hasAdminConfig(projectId),
      ]);
      setSidebarSpec(spec);
      setHasAdminFile(hasFile);
    } catch (e) {
      console.error("Failed to load sidebar spec:", e);
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsLoading(false);
    }
  }, [projectId]);

  // Save the current spec to admin.json
  const saveToFile = useCallback(async () => {
    if (!projectId || !sidebarSpec) return;

    try {
      await api.writeSidebarSpec(projectId, sidebarSpec);
      setHasAdminFile(true);
    } catch (e) {
      console.error("Failed to save sidebar spec:", e);
      throw e;
    }
  }, [projectId, sidebarSpec]);

  // Load spec when project changes
  useEffect(() => {
    loadSpec();
  }, [loadSpec]);

  // Listen for admin config changes
  useEffect(() => {
    if (!projectId) return;

    const unlistenPromise = listen<AdminConfigChangedPayload>(
      "admin_config_changed",
      (event) => {
        // Only reload if this is for our project
        if (event.payload.project_id === projectId) {
          console.log("Admin config changed, reloading sidebar spec");
          loadSpec();
        }
      },
    );

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [projectId, loadSpec]);

  return {
    sidebarSpec,
    hasAdminFile,
    isLoading,
    error,
    reload: loadSpec,
    saveToFile,
  };
}
