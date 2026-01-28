import { Loader2, Link, AlertCircle, Check, X } from "lucide-react";
import { useEffect, useState } from "react";

import * as api from "../api";
import type { Group, Item, SidebarSpec } from "../specs/types";
import type { Project } from "../types";

import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";

/**
 * Template types that can be imported
 */
type TemplateType = "item" | "group" | "spec";

interface TemplatePayload {
  type: TemplateType;
  groupId?: string; // Required for items
  data: Item | Group | SidebarSpec;
}

interface ImportTemplateDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  initialUrl?: string;
  projects: Project[];
  selectedProjectId: string | null;
}

/**
 * Detect the type of template from the JSON structure
 */
function detectTemplateType(json: unknown): TemplatePayload | null {
  if (!json || typeof json !== "object") {
    return null;
  }

  const obj = json as Record<string, unknown>;

  // Check if it's a full SidebarSpec (has groups array at root)
  if (Array.isArray(obj.groups)) {
    return { type: "spec", data: obj as SidebarSpec };
  }

  // Check if it's a Group (has id and either items, itemsSource, or itemTemplate)
  if (
    obj.id &&
    (Array.isArray(obj.items) || obj.itemsSource || obj.itemTemplate)
  ) {
    return { type: "group", data: obj as Group };
  }

  // Check if it's an Item (has id and queries)
  if (obj.id && Array.isArray(obj.queries)) {
    return { type: "item", data: obj as Item };
  }

  // Check for wrapper format: { type: "item", groupId: "admin", item: {...} }
  if (obj.type === "item" && obj.item && obj.groupId) {
    return {
      type: "item",
      groupId: obj.groupId as string,
      data: obj.item as Item,
    };
  }

  // Check for wrapper format: { type: "group", group: {...} }
  if (obj.type === "group" && obj.group) {
    return { type: "group", data: obj.group as Group };
  }

  return null;
}

/**
 * Format the template for display
 */
function formatTemplatePreview(payload: TemplatePayload): string {
  if (payload.type === "item") {
    const item = payload.data as Item;
    return `Item: "${item.name || item.id}"`;
  } else if (payload.type === "group") {
    const group = payload.data as Group;
    const itemCount = group.items?.length || 0;
    return `Group: "${group.name || group.id}"${itemCount > 0 ? ` (${itemCount} items)` : ""}`;
  } else {
    const spec = payload.data as SidebarSpec;
    return `Full Sidebar: ${spec.groups.length} groups`;
  }
}

export function ImportTemplateDialog({
  open,
  onOpenChange,
  initialUrl = "",
  projects,
  selectedProjectId,
}: ImportTemplateDialogProps) {
  const [url, setUrl] = useState(initialUrl);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [template, setTemplate] = useState<TemplatePayload | null>(null);
  const [targetProjectId, setTargetProjectId] = useState<string>(
    selectedProjectId || projects[0]?.id || ""
  );
  const [targetGroupId, setTargetGroupId] = useState<string>("admin");
  const [isImporting, setIsImporting] = useState(false);
  const [importSuccess, setImportSuccess] = useState(false);

  // Reset state when dialog opens/closes or initialUrl changes
  useEffect(() => {
    if (open) {
      setUrl(initialUrl);
      setError(null);
      setTemplate(null);
      setImportSuccess(false);
      setTargetProjectId(selectedProjectId || projects[0]?.id || "");

      // Auto-fetch if URL is provided
      if (initialUrl) {
        fetchTemplate(initialUrl);
      }
    }
  }, [open, initialUrl, selectedProjectId, projects]);

  const fetchTemplate = async (templateUrl: string) => {
    if (!templateUrl.trim()) {
      setError("Please enter a URL");
      return;
    }

    setIsLoading(true);
    setError(null);
    setTemplate(null);

    try {
      const response = await fetch(templateUrl, {
        method: "GET",
      });

      if (!response.ok) {
        throw new Error(
          `Failed to fetch: ${response.status} ${response.statusText}`
        );
      }

      const json = await response.json();
      const detected = detectTemplateType(json);

      if (!detected) {
        throw new Error(
          "Invalid template format. Expected an Item, Group, or SidebarSpec."
        );
      }

      // If it's an item with a groupId in the wrapper, use that
      if (detected.groupId) {
        setTargetGroupId(detected.groupId);
      }

      setTemplate(detected);
    } catch (err) {
      console.error("Failed to fetch template:", err);
      setError(err instanceof Error ? err.message : "Failed to fetch template");
    } finally {
      setIsLoading(false);
    }
  };

  const handleImport = async () => {
    if (!template || !targetProjectId) return;

    setIsImporting(true);
    setError(null);

    try {
      if (template.type === "item") {
        await api.addSidebarItem(
          targetProjectId,
          targetGroupId,
          template.data as Item
        );
      } else if (template.type === "group") {
        await api.addSidebarGroup(targetProjectId, template.data as Group);
      } else {
        // For full spec, we replace the entire thing
        await api.writeSidebarSpec(
          targetProjectId,
          template.data as SidebarSpec
        );
      }

      setImportSuccess(true);

      // Close dialog after a short delay
      setTimeout(() => {
        onOpenChange(false);
      }, 1500);
    } catch (err) {
      console.error("Failed to import template:", err);
      setError(
        err instanceof Error ? err.message : "Failed to import template"
      );
    } finally {
      setIsImporting(false);
    }
  };

  const targetProject = projects.find((p) => p.id === targetProjectId);

  if (!open) return null;

  return (
    <div className="absolute inset-0 bg-background/80 backdrop-blur-sm flex items-center justify-center z-50">
      <div className="bg-background border rounded-2xl p-6 w-full max-w-lg mx-4 shadow-xl max-h-[80vh] overflow-auto">
        {/* Header */}
        <div className="flex items-center justify-between mb-4">
          <div>
            <h2 className="text-lg font-semibold">Import Template</h2>
            <p className="text-sm text-muted-foreground">
              Import a sidebar item, group, or full configuration from a URL.
            </p>
          </div>
          <button
            onClick={() => onOpenChange(false)}
            className="text-muted-foreground hover:text-foreground p-1"
          >
            <X size={16} />
          </button>
        </div>

        <div className="space-y-4">
          {/* URL Input */}
          <div className="space-y-2">
            <Label htmlFor="url">Template URL</Label>
            <div className="flex gap-2">
              <Input
                id="url"
                placeholder="https://example.com/template.json"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    fetchTemplate(url);
                  }
                }}
              />
              <Button
                variant="secondary"
                onClick={() => fetchTemplate(url)}
                disabled={isLoading || !url.trim()}
              >
                {isLoading ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Link className="h-4 w-4" />
                )}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              Paste a URL to a JSON file containing an Item, Group, or full
              SidebarSpec.
            </p>
          </div>

          {/* Error Display */}
          {error && (
            <div className="flex items-center gap-2 text-sm text-destructive bg-destructive/10 p-3 rounded-md">
              <AlertCircle className="h-4 w-4 flex-shrink-0" />
              <span>{error}</span>
            </div>
          )}

          {/* Template Preview */}
          {template && (
            <div className="space-y-4">
              <div className="bg-muted/50 p-3 rounded-md">
                <p className="text-sm font-medium">
                  {formatTemplatePreview(template)}
                </p>
                <p className="text-xs text-muted-foreground mt-1">
                  Type: {template.type}
                </p>
              </div>

              {/* Project Selection */}
              <div className="space-y-2">
                <Label>Target Project</Label>
                <Select
                  value={targetProjectId}
                  onValueChange={setTargetProjectId}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="Select a project" />
                  </SelectTrigger>
                  <SelectContent>
                    {projects.map((project) => (
                      <SelectItem key={project.id} value={project.id}>
                        {project.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {/* Group Selection (only for items) */}
              {template.type === "item" && (
                <div className="space-y-2">
                  <Label>Target Group</Label>
                  <Input
                    value={targetGroupId}
                    onChange={(e) => setTargetGroupId(e.target.value)}
                    placeholder="admin"
                  />
                  <p className="text-xs text-muted-foreground">
                    The group ID where this item will be added.
                  </p>
                </div>
              )}

              {/* Warning for full spec replacement */}
              {template.type === "spec" && (
                <div className="flex items-center gap-2 text-sm text-yellow-500 bg-yellow-500/10 p-3 rounded-md">
                  <AlertCircle className="h-4 w-4 flex-shrink-0" />
                  <span>
                    This will replace your entire sidebar configuration for "
                    {targetProject?.name}".
                  </span>
                </div>
              )}
            </div>
          )}

          {/* Success Message */}
          {importSuccess && (
            <div className="flex items-center gap-2 text-sm text-green-500 bg-green-500/10 p-3 rounded-md">
              <Check className="h-4 w-4 flex-shrink-0" />
              <span>Template imported successfully!</span>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-2 mt-6">
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            onClick={handleImport}
            disabled={!template || isImporting || importSuccess}
          >
            {isImporting ? (
              <>
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                Importing...
              </>
            ) : importSuccess ? (
              <>
                <Check className="mr-2 h-4 w-4" />
                Imported!
              </>
            ) : (
              "Import"
            )}
          </Button>
        </div>
      </div>
    </div>
  );
}
