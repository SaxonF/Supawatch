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
  RefreshCw,
  Sprout,
  Trash2,
} from "lucide-react";
import { useState } from "react";
import * as api from "../api";
import type { Project } from "../types";
import { Button } from "./ui/button";

interface ProjectHeaderProps {
  project: Project;
  onUpdate: () => void;
  onDelete: () => void;
  showDiffSidebar: boolean;
  onToggleDiffSidebar: () => void;
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
}

export function ProjectHeader({
  project,
  onUpdate,
  onDelete,
  showDiffSidebar,
  onToggleDiffSidebar,
  sidebarCollapsed,
  onToggleSidebar,
}: ProjectHeaderProps) {
  const [isWatching, setIsWatching] = useState(project.is_watching);
  const [isLoading, setIsLoading] = useState(false);

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

  const handlePull = async () => {
    const confirmed = await ask(
      `Overwrite local changes for "${project.name}"? This cannot be undone.`,
      {
        title: "Confirm Pull",
        kind: "warning",
        okLabel: "Overwrite",
        cancelLabel: "Cancel",
      }
    );

    if (!confirmed) return;
    setIsLoading(true);
    try {
      await api.pullProject(project.id);
      await ask("Project pulled successfully", {
        title: "Success",
        kind: "info",
      });
    } catch (err) {
      console.error("Failed to pull project:", err);
      await ask("Failed to pull project: " + String(err), {
        title: "Error",
        kind: "error",
      });
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

  const handleRunSeeds = async () => {
    setIsLoading(true);
    try {
      const result = await api.runSeeds(project.id);
      await ask(result, {
        title: "Seeds Executed",
        kind: "info",
      });
    } catch (err) {
      console.error("Failed to run seeds:", err);
      await ask("Failed to run seeds: " + String(err), {
        title: "Error",
        kind: "error",
      });
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <header className="shrink-0 flex items-center justify-between px-5 py-3 border-b bg-background">
      <div className="flex items-center gap-3">
        <Button
          variant="ghost"
          size="icon"
          onClick={onToggleSidebar}
          className="text-muted-foreground hover:text-primary"
          title={sidebarCollapsed ? "Show sidebar" : "Hide sidebar"}
        >
          <PanelLeft size={18} />
        </Button>
        <div className="w-px h-5 bg-border" />
        <span
          className={`w-2.5 h-2.5 rounded-full ${
            isWatching
              ? "bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]"
              : "bg-muted-foreground/30"
          }`}
        />
        <h1 className="text-lg font-semibold">{project.name}</h1>
        {project.supabase_project_ref && (
          <span className="text-sm text-muted-foreground">
            {project.supabase_project_ref}
          </span>
        )}

        <div className="flex items-center gap-1 ml-2">
          <Button
            variant="ghost"
            size="icon"
            onClick={handleOpenFolder}
            className="text-muted-foreground hover:text-primary h-7 w-7"
            title="Open folder in Finder"
          >
            <Folder size={16} />
          </Button>

          {project.supabase_project_ref && (
            <Button
              variant="ghost"
              size="icon"
              onClick={handleOpenSupabase}
              className="text-muted-foreground hover:text-primary h-7 w-7"
              title="Open Supabase Dashboard"
            >
              <ExternalLink size={16} />
            </Button>
          )}
        </div>
      </div>

      <div className="flex items-center gap-2">
        <div className="flex items-center gap-1 mr-2">
          <Button
            variant="ghost"
            size="icon"
            className="text-muted-foreground hover:text-primary"
            onClick={handlePull}
            disabled={isLoading}
            title="Pull from remote"
          >
            <CloudDownload size={18} />
          </Button>

          <Button
            variant="ghost"
            size="icon"
            className="text-muted-foreground hover:text-primary"
            onClick={handleRunSeeds}
            disabled={isLoading}
            title="Run seed files"
          >
            <Sprout size={18} />
          </Button>
        </div>

        <div className="flex items-center">
          <Button
            variant="outline"
            className="rounded-r-none border-r-0 px-3 hover:bg-muted"
            onClick={handlePush}
            disabled={isLoading}
            title="Push to remote"
          >
            <CloudUpload size={16} className="mr-2" />
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
            <FileDiff size={16} />
          </Button>
        </div>

        <Button
          variant="outline"
          className={
            isWatching
              ? "text-green-500 border-green-500/50 hover:bg-green-500/10"
              : "text-muted-foreground hover:text-primary"
          }
          onClick={toggleWatch}
          disabled={isLoading}
          title={isWatching ? "Stop watching" : "Start watching"}
        >
          {isLoading ? (
            <RefreshCw size={16} className="animate-spin mr-2" />
          ) : isWatching ? (
            <Eye size={16} className="mr-2" />
          ) : (
            <EyeOff size={16} className="mr-2" />
          )}
          Watch
        </Button>

        <div className="w-px h-5 bg-border mx-1" />

        <Button
          variant="ghost"
          size="icon"
          className="text-muted-foreground hover:text-red-500 hover:bg-red-500/10"
          onClick={handleDelete}
          title="Delete project"
        >
          <Trash2 size={18} />
        </Button>
      </div>
    </header>
  );
}
