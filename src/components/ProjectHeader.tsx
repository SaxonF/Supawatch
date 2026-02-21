import { ask } from "@tauri-apps/plugin-dialog";
import { open } from "@tauri-apps/plugin-shell";
import {
  CloudDownload,
  ExternalLink,
  Eye,
  EyeOff,
  FileDiff,
  Folder,
  MoreHorizontal,
  PanelLeft,
  Scissors,
  Sprout,
  Trash2,
} from "lucide-react";
import { useEffect, useState } from "react";
import * as api from "../api";
import type { Project } from "../types";
import { notify } from "../utils/notification";
import { Button } from "./ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "./ui/dropdown-menu";

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

  const handleSplitSchema = async () => {
    const confirmed = await ask(
      "Split schema.sql into categorized files? This will replace the monolithic file with numbered files (e.g. 00_extensions.sql, 04_tables.sql).",
      {
        title: "Split Schema",
        kind: "info",
        okLabel: "Split Schema File",
        cancelLabel: "Cancel",
      },
    );

    if (!confirmed) return;
    try {
      const files = await api.splitSchema(project.id);
      notify(
        "Success",
        `Schema split into ${files.length} files:\n${files.join("\n")}`,
      );
    } catch (err) {
      console.error("Failed to split schema:", err);
      notify("Error", "Failed to split schema: " + String(err));
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
          <Button
            variant="outline"
            className={
              showDiffSidebar
                ? "bg-muted text-primary hover:text-primary/80 gap-2"
                : "text-muted-foreground hover:text-primary gap-2"
            }
            onClick={onToggleDiffSidebar}
            disabled={isLoading}
            title={showDiffSidebar ? "Hide schema diff" : "Show schema diff"}
          >
            <FileDiff size={16} strokeWidth={1} />
            Push
          </Button>

          <Button
            variant="outline"
            size="icon"
            className={
              isWatching
                ? "text-primary border-primary/50 hover:bg-primary/10 hover:text-primary/75"
                : "text-muted-foreground hover:text-primary"
            }
            onClick={toggleWatch}
            disabled={isLoading}
            title={
              isWatching
                ? "Stop watching local folder"
                : "Watch local folder and automatically deploy changes (ideal for prototyping, not production)"
            }
          >
            {isWatching ? (
              <Eye size={16} strokeWidth={1} />
            ) : (
              <EyeOff size={16} strokeWidth={1} />
            )}
          </Button>
        </div>

        <div className="w-px h-5 bg-border mx-1" />

        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="ghost" size="icon" title="More actions">
              <MoreHorizontal size={16} strokeWidth={1} />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={handleSplitSchema}>
              <Scissors size={16} strokeWidth={1} />
              Split Schema
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem variant="destructive" onClick={handleDelete}>
              <Trash2 size={16} strokeWidth={1} />
              Delete Project
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </header>
  );
}
