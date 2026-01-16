import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";

import * as api from "./api";
import { CreateProjectForm } from "./components/CreateProjectForm";
import { DiffSidebar } from "./components/DiffSidebar";
import { ProjectHeader } from "./components/ProjectHeader";
import { ProjectLogs } from "./components/ProjectLogs";
import { Settings } from "./components/Settings";
import { Sidebar } from "./components/Sidebar";
import { SqlEditor } from "./components/SqlEditor";
import type { FileChange, Project } from "./types";

import "./App.css";

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
    if (!showDiffSidebar) setLogsExpanded(false);
    setShowDiffSidebar(!showDiffSidebar);
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

    return () => {
      unlistenFileChange.then((fn) => fn());
      unlistenConfirmation.then((fn) => fn());
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
          onSelectProject={setSelectedProjectId}
          onAddProject={() => setShowCreateForm(true)}
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
                sidebarCollapsed={sidebarCollapsed}
                onToggleSidebar={() => setSidebarCollapsed(!sidebarCollapsed)}
              />
              <div className="flex-1 flex overflow-hidden">
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

                {/* Diff Sidebar - Right */}
                {showDiffSidebar &&
                  (selectedProject.supabase_project_ref ||
                    selectedProject.supabase_project_id) && (
                    <div className="w-[450px] border-l bg-background flex flex-col overflow-hidden shrink-0">
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
              </div>
            </>
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

        {/* Create Project Modal */}

        {showCreateForm && (
          <div className="absolute inset-0 bg-background/80 backdrop-blur-sm flex items-center justify-center z-50">
            <div className="bg-background border rounded-2xl p-6 w-full max-w-lg mx-4 shadow-xl">
              <CreateProjectForm
                onCreated={handleProjectCreated}
                onCancel={() => setShowCreateForm(false)}
              />
            </div>
          </div>
        )}

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
