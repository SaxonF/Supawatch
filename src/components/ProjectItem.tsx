import { ask } from "@tauri-apps/plugin-dialog";
import { open } from "@tauri-apps/plugin-shell";
import {
  CloudDownload,
  CloudUpload,
  ExternalLink,
  Eye,
  EyeOff,
  Folder,
  RefreshCw,
  Trash2,
} from "lucide-react";
import { useState } from "react";
import * as api from "../api";
import type { Project } from "../types";
import { notify } from "../utils/notification";
import "./ProjectItem.css";
import { Button } from "./ui/button";

interface ProjectItemProps {
  project: Project;
  onUpdate: () => void;
  onDelete: () => void;
}

export function ProjectItem({ project, onUpdate, onDelete }: ProjectItemProps) {
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
        okLabel: "Overwrite Local",
        cancelLabel: "Cancel Pull",
      },
    );

    if (!confirmed) return;
    setIsLoading(true);
    try {
      await api.pullProject(project.id);
      notify("Success", "Project pulled successfully");
    } catch (err) {
      console.error("Failed to pull project:", err);
      notify("Error", "Failed to pull project: " + String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const handlePush = async () => {
    setIsLoading(true);
    try {
      const result = await api.pushProject(project.id);
      const hasChanges =
        result.migration_sql.trim() !== "" ||
        result.edge_function_results.length > 0;

      if (!hasChanges) {
        notify("Info", "No schema changes detected");
      } else {
        const hasErrors = result.edge_function_results.some(
          (r) => r.status === "error",
        );
        if (hasErrors) {
          notify(
            "Deployment Warning",
            "Some edge functions failed to deploy. Please check the logs.",
          );
        } else {
          notify("Success", "Schema changes pushed successfully");
        }
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
            okLabel: "Force Push Changes",
            cancelLabel: "Cancel Push",
          },
        );

        if (confirmed) {
          try {
            await api.pushProject(project.id, true);
            notify("Success", "Schema changes pushed successfully");
          } catch (retryErr) {
            console.error("Failed to push project (forced):", retryErr);
            notify("Error", "Failed to push project: " + String(retryErr));
          }
        }
      } else {
        console.error("Failed to push project:", err);
        notify("Error", "Failed to push project: " + String(err));
      }
    } finally {
      setIsLoading(false);
    }
  };

  const handleDelete = async () => {
    const confirmed = await ask(`Delete project "${project.name}"?`, {
      title: "Confirm Delete",
      kind: "warning",
      okLabel: "Delete Project",
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
        `https://supabase.com/dashboard/project/${project.supabase_project_ref}`,
      );
    } catch (err) {
      console.error("Failed to open Supabase dashboard:", err);
    }
  };

  return (
    <div
      className={`p-4 bg-muted/75 hover:bg-muted transition-colors flex items-center justify-between group ${
        isWatching ? "watching" : ""
      }`}
    >
      <div className="flex items-center gap-6 overflow-hidden">
        <div className="flex items-center gap-3">
          <div className="relative">
            <span
              className={`w-2 h-2 rounded-full block ${
                isWatching
                  ? "bg-green-500 shadow-[0_0_8px_rgba(34,197,94,0.6)]"
                  : "bg-muted-foreground/30"
              }`}
            />
          </div>
          <span className="font-semibold whitespace-nowrap">
            {project.name}
          </span>
          <div className="flex items-center gap-0">
            <Button
              variant="ghost"
              size="icon-sm"
              onClick={handleOpenFolder}
              className="text-muted-foreground hover:text-primary"
              title="Open folder in Finder"
            >
              <Folder size={14} />
            </Button>
            {project.supabase_project_ref && (
              <Button
                variant="ghost"
                size="icon-sm"
                onClick={handleOpenSupabase}
                className="text-muted-foreground hover:text-primary"
                title="Open Supabase Dashboard"
              >
                <ExternalLink size={14} />
              </Button>
            )}
          </div>
        </div>
      </div>

      <div className="flex items-center gap-0">
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
          onClick={handlePush}
          disabled={isLoading}
          title="Push to remote"
        >
          <CloudUpload size={18} />
        </Button>

        <Button
          variant="ghost"
          size="icon"
          className={
            isWatching
              ? "text-primary hover:bg-primary/10"
              : "text-muted-foreground hover:text-primary"
          }
          onClick={toggleWatch}
          disabled={isLoading}
          title={isWatching ? "Stop watching" : "Start watching"}
        >
          {isLoading ? (
            <RefreshCw size={18} className="animate-spin" />
          ) : isWatching ? (
            <Eye size={18} />
          ) : (
            <EyeOff size={18} />
          )}
        </Button>

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
    </div>
  );
}
