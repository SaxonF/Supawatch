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
      <div className="shrink-0 flex items-center justify-between pl-4 pr-3 py-3">
        <div className="flex items-center gap-2">
          <svg
            width="20"
            height="20"
            viewBox="0 0 20 20"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <rect width="1.81818" height="20" fill="white" />
            <rect x="3.63635" width="1.81818" height="20" fill="white" />
            <rect x="18.1818" width="1.81818" height="20" fill="white" />
            <rect
              x="7.27277"
              y="10.9092"
              width="1.81818"
              height="9.09091"
              fill="white"
            />
            <rect x="7.27277" width="1.81818" height="9.09091" fill="white" />
            <rect
              x="16.3636"
              width="1.81818"
              height="9.09091"
              transform="rotate(90 16.3636 0)"
              fill="white"
            />
            <rect
              x="16.3636"
              y="3.63672"
              width="1.81818"
              height="9.09091"
              transform="rotate(90 16.3636 3.63672)"
              fill="white"
            />
            <rect
              x="16.3636"
              y="7.27246"
              width="1.81818"
              height="9.09091"
              transform="rotate(90 16.3636 7.27246)"
              fill="white"
            />
            <rect
              x="10.9091"
              y="10.9092"
              width="1.81818"
              height="9.09091"
              fill="white"
            />
            <rect
              x="14.5454"
              y="10.9092"
              width="1.81818"
              height="9.09091"
              fill="white"
            />
          </svg>
        </div>
        <Button
          variant="ghost"
          size="icon"
          onClick={onOpenSettings}
          className="text-sidebar-foreground/60 hover:text-sidebar-foreground"
          title="Settings"
        >
          <Settings size={16} strokeWidth={1} />
        </Button>
      </div>

      {/* Projects List */}
      <div className="flex-1 overflow-auto">
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
                className={`w-full px-4 py-2.5 flex items-center gap-4 text-left transition-colors ${
                  selectedProjectId === project.id
                    ? "bg-primary/10 text-primary"
                    : "text-primary hover:bg-primary/10 hover:text-primary"
                }`}
              >
                <span
                  className={`w-2 h-2 rounded-full shrink-0 ${
                    project.is_watching
                      ? "bg-primary"
                      : "bg-muted-foreground/30"
                  }`}
                />
                <div className="flex-1 min-w-0">
                  <div className="font-semibold truncate">{project.name}</div>
                  {project.supabase_project_ref && (
                    <div className="text-foreground/50 truncate">
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
          <Plus size={16} strokeWidth={1} className="text-muted-foreground" />
          Add Project
        </Button>
      </div>
    </div>
  );
}
