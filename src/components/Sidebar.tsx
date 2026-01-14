import { Plus, Settings } from "lucide-react";
import type { Project } from "../types";
import { Button } from "./ui/button";

interface SidebarProps {
  projects: Project[];
  selectedProjectId: string | null;
  onSelectProject: (projectId: string) => void;
  onAddProject: () => void;
  onOpenSettings: () => void;
  collapsed?: boolean;
}

export function Sidebar({
  projects,
  selectedProjectId,
  onSelectProject,
  onAddProject,
  onOpenSettings,
  collapsed = false,
}: SidebarProps) {
  if (collapsed) {
    return null;
  }

  return (
    <div className="w-64 h-full flex flex-col bg-sidebar border-r border-sidebar-border shrink-0">
      {/* Header */}
      <div className="shrink-0 flex items-center justify-between px-4 py-3 border-b border-sidebar-border">
        <div className="flex items-center gap-2">
          <svg
            width="11"
            height="11"
            viewBox="0 0 11 11"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <rect width="1" height="11" fill="white" />
            <rect x="2" width="1" height="11" fill="white" />
            <rect x="10" width="1" height="11" fill="white" />
            <rect x="4" y="6" width="1" height="5" fill="white" />
            <rect x="4" width="1" height="5" fill="white" />
            <rect
              x="9"
              width="1"
              height="5"
              transform="rotate(90 9 0)"
              fill="white"
            />
            <rect
              x="9"
              y="2"
              width="1"
              height="5"
              transform="rotate(90 9 2)"
              fill="white"
            />
            <rect
              x="9"
              y="4"
              width="1"
              height="5"
              transform="rotate(90 9 4)"
              fill="white"
            />
            <rect x="6" y="6" width="1" height="5" fill="white" />
            <rect x="8" y="6" width="1" height="5" fill="white" />
          </svg>
          <span className="font-semibold text-sidebar-foreground">Supawatch</span>
        </div>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onOpenSettings}
          className="text-sidebar-foreground/60 hover:text-sidebar-foreground"
          title="Settings"
        >
          <Settings size={16} />
        </Button>
      </div>

      {/* Projects List */}
      <div className="flex-1 overflow-auto py-2">
        {projects.length === 0 ? (
          <div className="px-4 py-8 text-center text-muted-foreground text-sm">
            <p>No projects yet</p>
            <p className="text-xs mt-1">Add a project to get started</p>
          </div>
        ) : (
          <div className="space-y-0.5">
            {projects.map((project) => (
              <button
                key={project.id}
                onClick={() => onSelectProject(project.id)}
                className={`w-full px-4 py-2.5 flex items-center gap-3 text-left transition-colors ${
                  selectedProjectId === project.id
                    ? "bg-sidebar-accent text-sidebar-accent-foreground"
                    : "text-sidebar-foreground/80 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground"
                }`}
              >
                <span
                  className={`w-2 h-2 rounded-full shrink-0 ${
                    project.is_watching
                      ? "bg-green-500 shadow-[0_0_6px_rgba(34,197,94,0.5)]"
                      : "bg-muted-foreground/30"
                  }`}
                />
                <div className="flex-1 min-w-0">
                  <div className="font-medium truncate">{project.name}</div>
                  {project.supabase_project_ref && (
                    <div className="text-xs text-muted-foreground truncate">
                      {project.supabase_project_ref}
                    </div>
                  )}
                </div>
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Add Project Button */}
      <div className="shrink-0 p-3 border-t border-sidebar-border">
        <Button
          variant="outline"
          className="w-full justify-center gap-2"
          onClick={onAddProject}
        >
          <Plus size={16} />
          Add Project
        </Button>
      </div>
    </div>
  );
}
