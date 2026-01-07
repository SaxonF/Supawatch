import { useEffect, useState } from "react";
import * as api from "../api";
import { Organization, RemoteProject } from "../types";
import "./CreateProjectForm.css";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";

interface CreateProjectFormProps {
  onCreated: () => void;
  onCancel: () => void;
}

type Mode = "create" | "sync";

export function CreateProjectForm({
  onCreated,
  onCancel,
}: CreateProjectFormProps) {
  const [mode, setMode] = useState<Mode>("create");
  const [name, setName] = useState("");
  const [localPath, setLocalPath] = useState("");

  // Create Mode State
  const [orgs, setOrgs] = useState<Organization[]>([]);
  const [selectedOrgId, setSelectedOrgId] = useState("");

  // Template State
  const [templates, setTemplates] = useState<string[]>([]);
  const [selectedTemplate, setSelectedTemplate] = useState("none");
  const [isEmptyFolder, setIsEmptyFolder] = useState(false);

  // Sync Mode State
  const [remoteProjects, setRemoteProjects] = useState<RemoteProject[]>([]);
  const [selectedProjectId, setSelectedProjectId] = useState("");

  const [isLoading, setIsLoading] = useState(false);
  const [isFetchingData, setIsFetchingData] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setIsFetchingData(true);
    try {
      // Fetch templates
      const templatesList = await api.getTemplates().catch(() => []);
      setTemplates(templatesList);

      const hasToken = await api.hasAccessToken();
      if (hasToken) {
        // Fetch orgs and projects in parallel
        const [orgsList, projectsList] = await Promise.all([
          api.listOrganizations().catch(() => []),
          api.listRemoteProjects().catch(() => []),
        ]);

        setOrgs(orgsList);
        if (orgsList.length > 0) {
          setSelectedOrgId(orgsList[0].id);
        }

        setRemoteProjects(projectsList);
        if (projectsList.length > 0) {
          setSelectedProjectId(projectsList[0].id);
        }
      }
    } catch (err) {
      console.error("Failed to load data", err);
    } finally {
      setIsFetchingData(false);
    }
  };

  const selectFolder = async () => {
    try {
      const selected = await api.pickProjectFolder();

      if (selected) {
        const path = selected;
        setLocalPath(path);

        // Check if empty
        const empty = await api.isFolderEmpty(path);
        setIsEmptyFolder(empty);

        // Auto-fill name from folder name if empty and in Create mode
        if (mode === "create" && !name) {
          const folderName = path.split("/").pop() || "";
          setName(folderName);
        }
      }
    } catch (err) {
      console.error("Failed to select folder:", err);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!localPath.trim()) {
      setError("Local path is required");
      return;
    }

    if (mode === "create") {
      if (!name.trim()) {
        setError("Project name is required");
        return;
      }
      if (!selectedOrgId) {
        setError("Organization is required");
        return;
      }
    } else {
      if (!selectedProjectId) {
        setError("Please select a project to sync");
        return;
      }
    }

    setIsLoading(true);
    try {
      if (mode === "create") {
        // Copy template if selected and folder is empty
        if (isEmptyFolder && selectedTemplate !== "none") {
          await api.copyTemplate(selectedTemplate, localPath.trim());
        }

        await api.createProject(
          name.trim(),
          localPath.trim(),
          undefined,
          undefined, // ref not needed for new
          selectedOrgId
        );
      } else {
        // Sync Mode
        const project = remoteProjects.find((p) => p.id === selectedProjectId);
        if (!project) throw new Error("Selected project not found");

        await api.createProject(
          project.name, // Use existing name
          localPath.trim(),
          undefined, // ID not strictly needed if we have ref?
          project.id, // Assuming ID is Ref based on previous analysis
          undefined // Org not needed for link
        );
      }
      onCreated();
    } catch (err) {
      setError(String(err));
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <form className="create-project-form" onSubmit={handleSubmit}>
      <div className="flex items-center gap-4 mb-4">
        <Button
          type="button"
          variant="ghost"
          size="lg"
          className={
            mode === "create"
              ? "text-primary p-0 hover:bg-transparent"
              : "text-muted-foreground p-0 hover:bg-transparent"
          }
          onClick={() => setMode("create")}
        >
          Create New
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="lg"
          className={
            mode === "sync"
              ? "text-primary p-0 hover:bg-transparent"
              : "text-muted-foreground p-0 hover:bg-transparent"
          }
          onClick={() => setMode("sync")}
        >
          Sync Existing
        </Button>
      </div>
      <div className="rounded-xl overflow-hidden border border-border divider divider-border mb-4">
        {mode === "create" && (
          <>
            <div className="grid grid-cols-[1fr_2fr] items-center gap-2 bg-muted/75 hover:bg-muted p-3 border-b">
              <label htmlFor="org">Organization</label>
              <Select
                value={selectedOrgId}
                onValueChange={setSelectedOrgId}
                disabled={isFetchingData}
              >
                <SelectTrigger id="org" className="w-full truncate">
                  <SelectValue
                    placeholder="Select organization"
                    className="truncate w-full"
                  />
                </SelectTrigger>
                <SelectContent>
                  {orgs.map((org) => (
                    <SelectItem key={org.id} value={org.id}>
                      {org.name}
                    </SelectItem>
                  ))}
                  {orgs.length === 0 && (
                    <SelectItem value="none" disabled>
                      No organizations found
                    </SelectItem>
                  )}
                </SelectContent>
              </Select>
            </div>
            <div className="grid grid-cols-[1fr_2fr] items-center gap-2 bg-muted/75 hover:bg-muted p-3 border-b">
              <label htmlFor="name">Project Name</label>
              <Input
                id="name"
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="My Supabase Project"
                autoFocus
              />
            </div>
          </>
        )}

        {mode === "sync" && (
          <div className="grid grid-cols-[1fr_2fr] items-center gap-2 bg-muted/75 hover:bg-muted p-3 border-b">
            <label htmlFor="project">Project</label>
            <Select
              value={selectedProjectId}
              onValueChange={setSelectedProjectId}
              disabled={isFetchingData}
            >
              <SelectTrigger id="project" className="w-full">
                <SelectValue placeholder="Select project" />
              </SelectTrigger>
              <SelectContent>
                {remoteProjects.map((p) => (
                  <SelectItem key={p.id} value={p.id}>
                    {p.name} ({p.id})
                  </SelectItem>
                ))}
                {remoteProjects.length === 0 && (
                  <SelectItem value="none" disabled>
                    No projects found
                  </SelectItem>
                )}
              </SelectContent>
            </Select>
          </div>
        )}

        <div className="grid grid-cols-[1fr_2fr] items-center gap-2 bg-muted/75 hover:bg-muted p-3">
          <label htmlFor="path">Local Folder</label>
          <Button
            type="button"
            variant="outline"
            onClick={selectFolder}
            className="max-w-full justify-start font-normal truncate"
          >
            <span className="truncate">{localPath ? localPath : "Browse"}</span>
          </Button>
        </div>

        {mode === "create" && isEmptyFolder && templates.length > 0 && (
          <div className="grid grid-cols-[1fr_2fr] items-center gap-2 bg-muted/75 hover:bg-muted p-3 border-t">
            <label htmlFor="template">Template</label>
            <Select
              value={selectedTemplate}
              onValueChange={setSelectedTemplate}
            >
              <SelectTrigger id="template" className="w-full">
                <SelectValue placeholder="Select template" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="none">None</SelectItem>
                {templates.map((t) => (
                  <SelectItem key={t} value={t}>
                    {t}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
        )}
      </div>

      {error && <div className="error">{error}</div>}

      <div className="flex items-center gap-2">
        <Button
          type="button"
          variant="outline"
          onClick={onCancel}
          className="h-12 flex-1 rounded-xl"
          disabled={isLoading}
        >
          Cancel
        </Button>
        <Button
          type="submit"
          className="h-12 flex-1 rounded-xl"
          disabled={isLoading}
        >
          {isLoading
            ? mode === "create"
              ? "Creating..."
              : "Syncing..."
            : mode === "create"
            ? "Create Project"
            : "Sync Project"}
        </Button>
      </div>
    </form>
  );
}
