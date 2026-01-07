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
import "./ProjectItem.css";

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
    if (
      !confirm(
        `Overwrite local changes for "${project.name}"? This cannot be undone.`
      )
    )
      return;
    setIsLoading(true);
    try {
      await api.pullProject(project.id);
      alert("Project pulled successfully");
    } catch (err) {
      console.error("Failed to pull project:", err);
      alert("Failed to pull project: " + String(err));
    } finally {
      setIsLoading(false);
    }
  };

  const handlePush = async () => {
    setIsLoading(true);
    try {
      const result = await api.pushProject(project.id);
      if (result === "No changes") {
        alert("No schema changes detected");
      } else {
        alert("Schema changes pushed successfully");
      }
    } catch (err) {
      const errorMsg = String(err);
      if (errorMsg.startsWith("CONFIRMATION_NEEDED:")) {
        const summary = errorMsg.replace("CONFIRMATION_NEEDED:", "");
        if (
          confirm(
            `Destructive changes detected!\n${summary}\n\nAre you sure you want to proceed? This cannot be undone.`
          )
        ) {
          try {
            await api.pushProject(project.id, true);
            alert("Schema changes pushed successfully");
          } catch (retryErr) {
            console.error("Failed to push project (forced):", retryErr);
            alert("Failed to push project: " + String(retryErr));
          }
        }
      } else {
        console.error("Failed to push project:", err);
        alert("Failed to push project: " + String(err));
      }
    } finally {
      setIsLoading(false);
    }
  };

  const handleDelete = async () => {
    if (!confirm(`Delete project "${project.name}"?`)) return;
    try {
      await api.deleteProject(project.id);
      onDelete();
    } catch (err) {
      console.error("Failed to delete project:", err);
    }
  };

  const handleOpenFolder = async () => {
    try {
      await open(project.local_path);
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
          <div className="flex items-center gap-1 ml-1">
            <button
              onClick={handleOpenFolder}
              className="p-1 text-muted-foreground hover:text-primary transition-colors cursor-pointer"
              title="Open folder in Finder"
            >
              <Folder size={14} />
            </button>
            {project.supabase_project_ref && (
              <button
                onClick={handleOpenSupabase}
                className="p-1 text-muted-foreground hover:text-primary transition-colors cursor-pointer"
                title="Open Supabase Dashboard"
              >
                <ExternalLink size={14} />
              </button>
            )}
          </div>
        </div>
      </div>

      <div className="flex items-center gap-2">
        <button
          className="p-2 text-muted-foreground hover:text-primary hover:bg-muted rounded-lg transition-colors cursor-pointer"
          onClick={handlePull}
          disabled={isLoading}
          title="Pull from remote"
        >
          <CloudDownload size={18} />
        </button>

        <button
          className="p-2 text-muted-foreground hover:text-primary hover:bg-muted rounded-lg transition-colors cursor-pointer"
          onClick={handlePush}
          disabled={isLoading}
          title="Push to remote"
        >
          <CloudUpload size={18} />
        </button>

        <button
          className={`p-2 rounded-lg transition-colors cursor-pointer ${
            isWatching
              ? "text-primary hover:bg-primary/10"
              : "text-muted-foreground hover:text-primary hover:bg-muted"
          }`}
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
        </button>

        <button
          className="p-2 text-muted-foreground hover:text-red-500 hover:bg-red-500/10 rounded-lg transition-colors cursor-pointer"
          onClick={handleDelete}
          title="Delete project"
        >
          <Trash2 size={18} />
        </button>
      </div>
    </div>
  );
}
