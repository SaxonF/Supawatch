import { ask } from "@tauri-apps/plugin-dialog";
import { open } from "@tauri-apps/plugin-shell";
import {
  CloudDownload,
  CloudUpload,
  ExternalLink,
  Eye,
  EyeOff,
  FileDiff,
  Folder,
  PanelLeft,
  Sprout,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import * as api from "../api";
import type { Project } from "../types";
import { Button } from "./ui/button";

interface ProjectHeaderProps {
  project: Project;
  onUpdate: () => void;
  onDelete: () => void;
  showDiffSidebar: boolean;
  onToggleDiffSidebar: () => void;
  showSeedSidebar: boolean;
  onToggleSeedSidebar: () => void;
  showPullSidebar: boolean;
  onTogglePullSidebar: () => void;
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
}

export function ProjectHeader({
  project,
  onUpdate,
  onDelete,
  showDiffSidebar,
  onToggleDiffSidebar,
  showSeedSidebar,
  onToggleSeedSidebar,
  showPullSidebar,
  onTogglePullSidebar,
  sidebarCollapsed,
  onToggleSidebar,
}: ProjectHeaderProps) {
  const [isWatching, setIsWatching] = useState(project.is_watching);
  const [isLoading, setIsLoading] = useState(false);

  // Sync isWatching state when project changes
  useEffect(() => {
    setIsWatching(project.is_watching);
  }, [project.id, project.is_watching]);

  const toggleWatch = async () => {
    setIsLoading(true);
    try {
      if (isWatching) {
        await api.stopWatching(project.id);
      } else {
        await api.startWatching(project.id);
      }
      setIsWatching(!isWatching);
      onUpdate();
    } catch (err) {
      console.error("Failed to toggle watch:", err);
    } finally {
      setIsLoading(false);
    }
  };

  const handlePush = async () => {
    setIsLoading(true);
    try {
      const result = await api.pushProject(project.id);
      if (result === "No changes") {
        await ask("No schema changes detected", {
          title: "Info",
          kind: "info",
        });
      } else {
        await ask("Schema changes pushed successfully", {
          title: "Success",
          kind: "info",
        });
      }
    } catch (err) {
      const errorMsg = String(err);
      if (errorMsg.startsWith("CONFIRMATION_NEEDED:")) {
        const summary = errorMsg.replace("CONFIRMATION_NEEDED:", "");
        const confirmed = await ask(
          `Destructive changes detected!\n\n${summary}\n\nDo you want to force push these changes?`,
          {
            title: "Destructive Changes Detected",
            kind: "warning",
            okLabel: "Push Changes",
            cancelLabel: "Cancel",
          }
        );

        if (confirmed) {
          try {
            await api.pushProject(project.id, true);
            await ask("Schema changes pushed successfully", {
              title: "Success",
              kind: "info",
            });
          } catch (retryErr) {
            console.error("Failed to push project (forced):", retryErr);
            await ask("Failed to push project: " + String(retryErr), {
              title: "Error",
              kind: "error",
            });
          }
        }
      } else {
        console.error("Failed to push project:", err);
        await ask("Failed to push project: " + String(err), {
          title: "Error",
          kind: "error",
        });
      }
    } finally {
      setIsLoading(false);
    }
  };

  const handleDelete = async () => {
    const confirmed = await ask(`Delete project "${project.name}"?`, {
      title: "Confirm Delete",
      kind: "warning",
      okLabel: "Delete",
      cancelLabel: "Cancel",
    });

    if (!confirmed) return;
    try {
      await api.deleteProject(project.id);
      onDelete();
    } catch (err) {
      console.error("Failed to delete project:", err);
    }
  };

  const handleOpenFolder = async () => {
    try {
      await api.revealInFinder(project.local_path);
    } catch (err) {
      console.error("Failed to open folder:", err);
    }
  };

  const handleOpenSupabase = async () => {
    if (!project.supabase_project_ref) return;
    try {
      await open(
        `https://supabase.com/dashboard/project/${project.supabase_project_ref}`
      );
    } catch (err) {
      console.error("Failed to open Supabase dashboard:", err);
    }
  };

  return (
    <header className="shrink-0 flex items-center justify-between px-5 py-3 border-b bg-muted/10">
      <div className="flex items-center gap-4">
        <Button
          variant="ghost"
          size="icon"
          onClick={onToggleSidebar}
          title={sidebarCollapsed ? "Show sidebar" : "Hide sidebar"}
        >
          <PanelLeft size={16} strokeWidth={1} />
        </Button>
        <div className="w-px h-5 bg-border" />
        <span
          className={`w-2 h-2 rounded-full ${
            isWatching
              ? "bg-primary shadow-[0_0_8px_rgba(34,197,94,0.6)]"
              : "bg-muted-foreground/30"
          }`}
        />
        <div className="flex items-center gap-2 mb-1">
          <h1 className="font-semibold">{project.name}</h1>
          {project.supabase_project_ref && (
            <span className="text-muted-foreground">
              {project.supabase_project_ref}
            </span>
          )}
        </div>

        <div className="flex items-center gap-0">
          <Button
            variant="ghost"
            size="icon"
            onClick={handleOpenFolder}
            title="Open folder in Finder"
          >
            <Folder size={16} strokeWidth={1} />
          </Button>

          {project.supabase_project_ref && (
            <Button
              variant="ghost"
              size="icon"
              onClick={handleOpenSupabase}
              title="Open Supabase Dashboard"
            >
              <ExternalLink size={16} strokeWidth={1} />
            </Button>
          )}
        </div>
      </div>

      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="icon"
            onClick={handleDelete}
            title="Delete project"
          >
            <Trash2 size={16} strokeWidth={1} />
          </Button>
          <Button
            variant="outline"
            size="icon"
            className={
              showSeedSidebar
                ? "bg-muted text-primary hover:text-primary/80"
                : "hover:text-primary"
            }
            onClick={onToggleSeedSidebar}
            disabled={isLoading}
            title={showSeedSidebar ? "Hide seed files" : "Show seed files"}
          >
            <Sprout size={16} strokeWidth={1} />
          </Button>

          <Button
            variant="outline"
            size="icon"
            className={
              showPullSidebar
                ? "bg-muted text-primary hover:text-primary/80"
                : "hover:text-primary"
            }
            onClick={onTogglePullSidebar}
            disabled={isLoading}
            title={showPullSidebar ? "Hide remote content" : "Pull from remote"}
          >
            <CloudDownload size={16} strokeWidth={1} />
          </Button>
        </div>
        <div className="w-px h-5 bg-border mx-1" />
        <div className="flex items-center gap-2">
          <div className="flex items-center">
            <Button
              variant="outline"
              className="rounded-r-none border-r-0 px-3 hover:bg-muted"
              onClick={handlePush}
              disabled={isLoading}
              title="Push to remote"
            >
              <CloudUpload size={16} strokeWidth={1} />
              Push
            </Button>
            <Button
              variant="outline"
              size="icon"
              className={`rounded-l-none w-9 ${
                showDiffSidebar
                  ? "bg-muted text-primary hover:text-primary/80"
                  : "text-muted-foreground hover:text-primary hover:bg-muted"
              }`}
              onClick={onToggleDiffSidebar}
              disabled={isLoading}
              title={showDiffSidebar ? "Hide schema diff" : "Show schema diff"}
            >
              <FileDiff size={16} strokeWidth={1} />
            </Button>
          </div>

          <Button
            variant="outline"
            className={
              isWatching
                ? "text-primary border-primary/50 hover:bg-primary/10 hover:text-primary/75"
                : "text-muted-foreground hover:text-primary"
            }
            onClick={toggleWatch}
            disabled={isLoading}
            title={isWatching ? "Stop watching" : "Start watching"}
          >
            {isWatching ? (
              <Eye size={16} strokeWidth={1} />
            ) : (
              <EyeOff size={16} strokeWidth={1} />
            )}
            Watch
          </Button>
        </div>
      </div>
    </header>
  );
}
