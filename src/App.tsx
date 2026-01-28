import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { useCallback, useEffect, useState } from "react";

import * as api from "./api";
import { CreateProjectForm } from "./components/CreateProjectForm";
import { DiffSidebar } from "./components/DiffSidebar";
import { ProjectHeader } from "./components/ProjectHeader";
import { ProjectLogs } from "./components/ProjectLogs";
import { PullSidebar } from "./components/PullSidebar";
import { SeedSidebar } from "./components/SeedSidebar";
import { Settings } from "./components/Settings";
import { Sidebar } from "./components/Sidebar";
import { SqlEditor } from "./components/SqlEditor";
import type { Group, Item } from "./specs/types";
import type { FileChange, Project } from "./types";

import "./App.css";

/**
 * Parse and handle deeplink URLs for adding items/groups to admin.json
 * URL format:
 *   supawatch://add-item?projectId=xxx&groupId=admin&item={...encoded JSON...}
 *   supawatch://add-group?projectId=xxx&group={...encoded JSON...}
 */
async function handleDeeplink(url: string): Promise<void> {
  try {
    const parsed = new URL(url);
    const action = parsed.hostname; // e.g., "add-item" or "add-group"
    const params = parsed.searchParams;

    const projectId = params.get("projectId");
    if (!projectId) {
      console.error("Deeplink missing projectId");
      return;
    }

    if (action === "add-item") {
      const groupId = params.get("groupId");
      const itemJson = params.get("item");

      if (!groupId || !itemJson) {
        console.error("Deeplink add-item missing groupId or item");
        return;
      }

      const item: Item = JSON.parse(decodeURIComponent(itemJson));
      await api.addSidebarItem(projectId, groupId, item);
      console.log("Added sidebar item via deeplink:", item.id);
    } else if (action === "add-group") {
      const groupJson = params.get("group");

      if (!groupJson) {
        console.error("Deeplink add-group missing group");
        return;
      }

      const group: Group = JSON.parse(decodeURIComponent(groupJson));
      await api.addSidebarGroup(projectId, group);
      console.log("Added sidebar group via deeplink:", group.id);
    } else {
      console.log("Unknown deeplink action:", action);
    }
  } catch (err) {
    console.error("Failed to handle deeplink:", err);
  }
}

function App() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(
    null
  );
  const [isLoading, setIsLoading] = useState(true);
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [logsExpanded, setLogsExpanded] = useState(false);
  const [showDiffSidebar, setShowDiffSidebar] = useState(false);
  const [showSeedSidebar, setShowSeedSidebar] = useState(false);
  const [showPullSidebar, setShowPullSidebar] = useState(false);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);

  const selectedProject =
    projects.find((p) => p.id === selectedProjectId) || null;

  const loadProjects = async () => {
    // ... same ...
    try {
      const data = await api.getProjects();
      setProjects(data);

      // Auto-select first project if none selected
      if (!selectedProjectId && data.length > 0) {
        setSelectedProjectId(data[0].id);
      }
      // Clear selection if selected project no longer exists
      if (selectedProjectId && !data.find((p) => p.id === selectedProjectId)) {
        setSelectedProjectId(data.length > 0 ? data[0].id : null);
      }
    } catch (err) {
      console.error("Failed to load projects:", err);
    }
  };

  const toggleDiffSidebar = () => {
    if (!showDiffSidebar) {
      setLogsExpanded(false);
      setShowSeedSidebar(false);
    }
    setShowDiffSidebar(!showDiffSidebar);
  };

  const toggleSeedSidebar = () => {
    if (!showSeedSidebar) {
      setLogsExpanded(false);
      setShowDiffSidebar(false);
    }
    setShowSeedSidebar(!showSeedSidebar);
  };

  const togglePullSidebar = () => {
    if (!showPullSidebar) {
      setLogsExpanded(false);
      setShowDiffSidebar(false);
      setShowSeedSidebar(false);
    }
    setShowPullSidebar(!showPullSidebar);
  };

  useEffect(() => {
    // ... same ...
    const initialize = async () => {
      invoke("init");

      // Check if we have an access token, if not show settings
      const hasToken = await api.hasAccessToken();
      if (!hasToken) {
        setShowSettings(true);
      }

      await loadProjects();
      setIsLoading(false);
    };

    initialize();

    // Listen for file changes
    const unlistenFileChange = listen<FileChange>("file_change", (event) => {
      console.log("File changed:", event.payload);
    });

    const unlistenConfirmation = listen<{
      project_id: string;
      summary: string;
    }>("schema-push-confirmation-needed", async (event) => {
      // ... same ...
      const confirmed = await ask(
        `Destructive changes detected during auto-push!\n\n${event.payload.summary}\n\nDo you want to force push these changes?`,
        {
          title: "Destructive Changes Detected",
          kind: "warning",
          okLabel: "Push Changes",
          cancelLabel: "Cancel",
        }
      );

      if (confirmed) {
        try {
          await api.pushProject(event.payload.project_id, true);
          console.log("Forced push successful");
        } catch (err) {
          console.error("Failed to push project (forced):", err);
          await ask(`Failed to push project: ${err}`, {
            title: "Push Failed",
            kind: "error",
          });
        }
      }
    });

    // Listen for deeplink events (for adding items/groups to admin.json)
    // This will be triggered when the app is opened via a supawatch:// URL
    const unlistenDeeplink = listen<{ url: string }>("deeplink", (event) => {
      console.log("Deeplink received:", event.payload.url);
      handleDeeplink(event.payload.url);
    });

    return () => {
      unlistenFileChange.then((fn) => fn());
      unlistenConfirmation.then((fn) => fn());
      unlistenDeeplink.then((fn) => fn());
    };
  }, []);

  const handleProjectCreated = () => {
    setShowCreateForm(false);
    loadProjects();
  };

  const handleProjectDeleted = () => {
    loadProjects();
  };

  if (isLoading) {
    return (
      <div className="dark h-full">
        <div className="bg-background h-full flex items-center justify-center text-muted-foreground">
          Loading...
        </div>
      </div>
    );
  }

  return (
    <div className="dark h-full">
      <div className="bg-background h-full flex">
        {/* Sidebar */}
        <Sidebar
          projects={projects}
          selectedProjectId={selectedProjectId}
          onSelectProject={(id) => {
            setSelectedProjectId(id);
            setShowCreateForm(false);
          }}
          onAddProject={() => {
            setShowCreateForm(true);
            setSelectedProjectId(null);
          }}
          onOpenSettings={() => setShowSettings(true)}
          collapsed={sidebarCollapsed}
        />

        {/* Main Content Area */}
        <div className="flex-1 flex flex-col overflow-hidden">
          {selectedProject ? (
            <>
              <ProjectHeader
                key={selectedProject.id}
                project={selectedProject}
                onUpdate={loadProjects}
                onDelete={handleProjectDeleted}
                showDiffSidebar={showDiffSidebar}
                onToggleDiffSidebar={toggleDiffSidebar}
                showSeedSidebar={showSeedSidebar}
                onToggleSeedSidebar={toggleSeedSidebar}
                showPullSidebar={showPullSidebar}
                onTogglePullSidebar={togglePullSidebar}
                sidebarCollapsed={sidebarCollapsed}
                onToggleSidebar={() => setSidebarCollapsed(!sidebarCollapsed)}
              />
              <div className="flex-1 flex overflow-hidden relative">
                {/* Main Content (SqlEditor + Logs) */}
                <div
                  className={`flex-1 flex overflow-hidden transition-opacity duration-300 ${
                    showDiffSidebar || showSeedSidebar || showPullSidebar
                      ? "opacity-25 pointer-events-none"
                      : ""
                  }`}
                >
                  {/* SQL Editor - Main Content */}
                  <div className="flex-1 overflow-hidden">
                    {selectedProject.supabase_project_id ? (
                      <SqlEditor projectId={selectedProject.id} />
                    ) : (
                      <div className="flex-1 flex items-center justify-center text-muted-foreground h-full">
                        <div className="text-center">
                          <p>Project not linked to Supabase</p>
                          <p className="text-sm mt-1">
                            SQL editor will be available once the project is
                            linked
                          </p>
                        </div>
                      </div>
                    )}
                  </div>

                  {/* Loans Sidebar - Right (Always rendered if project linked, handles its own width) */}
                  {selectedProject.supabase_project_id && (
                    <ProjectLogs
                      projectId={selectedProject.id}
                      expanded={logsExpanded}
                      onToggle={() => setLogsExpanded(!logsExpanded)}
                    />
                  )}
                </div>

                {/* Diff Sidebar - Overlay Sheet */}
                {showDiffSidebar &&
                  (selectedProject.supabase_project_ref ||
                    selectedProject.supabase_project_id) && (
                    <div className="absolute top-0 right-0 bottom-0 w-[450px] border-l bg-background flex flex-col overflow-hidden shrink-0 z-20 shadow-xl">
                      <DiffSidebar
                        projectId={selectedProject.id}
                        onClose={() => setShowDiffSidebar(false)}
                        onSuccess={() => {
                          // Optionally refresh project or something
                          // Probably nothing needs to be done since diff sidebar reloads on push success
                        }}
                      />
                    </div>
                  )}

                {/* Seed Sidebar - Overlay Sheet */}
                {showSeedSidebar && selectedProject && (
                  <div className="absolute top-0 right-0 bottom-0 w-[450px] border-l bg-background flex flex-col overflow-hidden shrink-0 z-20 shadow-xl">
                    <SeedSidebar
                      projectId={selectedProject.id}
                      onClose={() => setShowSeedSidebar(false)}
                    />
                  </div>
                )}

                {/* Pull Sidebar - Overlay Sheet */}
                {showPullSidebar && selectedProject && (
                  <div className="absolute top-0 right-0 bottom-0 w-[450px] border-l bg-background flex flex-col overflow-hidden shrink-0 z-20 shadow-xl">
                    <PullSidebar
                      projectId={selectedProject.id}
                      onClose={() => setShowPullSidebar(false)}
                      onSuccess={() => {
                        setShowPullSidebar(false);
                        // Maybe reload project / logs?
                        loadProjects();
                      }}
                    />
                  </div>
                )}
              </div>
            </>
          ) : showCreateForm ? (
            <div className="h-full overflow-auto">
              <div className="flex-1 flex items-center justify-center p-6 min-h-full">
                <div className="w-full max-w-lg">
                  <CreateProjectForm
                    onCreated={handleProjectCreated}
                    onCancel={() => setShowCreateForm(false)}
                  />
                </div>
              </div>
            </div>
          ) : (
            <div className="flex-1 flex items-center justify-center text-muted-foreground">
              <div className="text-center">
                <p>No project selected</p>
                <p className="text-sm mt-1">
                  Select a project from the sidebar or add a new one
                </p>
              </div>
            </div>
          )}
        </div>

        {/* Settings Modal */}
        {showSettings && (
          <div className="absolute inset-0 bg-background/80 backdrop-blur-sm flex items-center justify-center z-50">
            <div className="bg-background border rounded-2xl p-6 w-full max-w-lg mx-4 shadow-xl max-h-[80vh] overflow-auto">
              <div className="flex items-center justify-between mb-4">
                <h2 className="text-lg font-semibold">Settings</h2>
                <button
                  onClick={() => setShowSettings(false)}
                  className="text-muted-foreground hover:text-foreground p-1"
                >
                  <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                    <path
                      d="M12 4L4 12M4 4L12 12"
                      stroke="currentColor"
                      strokeWidth="1.5"
                      strokeLinecap="round"
                    />
                  </svg>
                </button>
              </div>
              <Settings />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export default App;
