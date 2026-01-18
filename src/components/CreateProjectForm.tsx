import { useForm } from "@tanstack/react-form";
import { zodValidator } from "@tanstack/zod-form-adapter";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { useEffect, useState } from "react";
import { z } from "zod";
import * as api from "../api";
import { Organization, RemoteProject } from "../types";
import { Button } from "./ui/button";
import {
  Field,
  FieldContent,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
  FieldTitle,
} from "./ui/field";
import { Input } from "./ui/input";
import { RadioGroup, RadioGroupItem } from "./ui/radio-group";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";
import { Switch } from "./ui/switch";

interface CreateProjectFormProps {
  onCreated: () => void;
  onCancel: () => void;
}

type Mode = "create" | "sync";

const modes = [
  {
    id: "create",
    title: "Create New",
    description: "Start fresh with a new project",
  },
  {
    id: "sync",
    title: "Sync Existing",
    description: "Link to an existing project",
  },
] as const;

const createSchema = z
  .object({
    name: z.string().min(1, "Project name is required"),
    orgId: z.string().min(1, "Organization is required"),
    localPath: z.string().min(1, "Local path is required"),
    template: z.string(),
    projectId: z.string(),
    generateTypescript: z.boolean(),
    typescriptOutputPath: z.string(),
  })
  .refine(
    (data) => {
      if (data.generateTypescript && !data.typescriptOutputPath) {
        return false;
      }
      return true;
    },
    {
      message: "Output path is required when TypeScript generation is enabled",
      path: ["typescriptOutputPath"],
    }
  );

const syncSchema = z
  .object({
    projectId: z.string().min(1, "Project is required"),
    localPath: z.string().min(1, "Local path is required"),
    orgId: z.string().min(1, "Organization is required"),
    name: z.string(), // Unused in sync but matches form state
    template: z.string(), // Unused in sync but matches form state
    generateTypescript: z.boolean(),
    typescriptOutputPath: z.string(),
  })
  .refine(
    (data) => {
      if (data.generateTypescript && !data.typescriptOutputPath) {
        return false;
      }
      return true;
    },
    {
      message: "Output path is required when TypeScript generation is enabled",
      path: ["typescriptOutputPath"],
    }
  );

export function CreateProjectForm({
  onCreated,
  onCancel,
}: CreateProjectFormProps) {
  const [mode, setMode] = useState<Mode>("create");
  const [isEmptyFolder, setIsEmptyFolder] = useState(false);
  const [isFetchingData, setIsFetchingData] = useState(false);

  // Data State
  const [orgs, setOrgs] = useState<Organization[]>([]);
  const [templates, setTemplates] = useState<string[]>([]);
  const [remoteProjects, setRemoteProjects] = useState<RemoteProject[]>([]);

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setIsFetchingData(true);
    try {
      const templatesList = await api.getTemplates().catch(() => []);
      setTemplates(templatesList);

      const hasToken = await api.hasAccessToken();
      if (hasToken) {
        const [orgsList, projectsList] = await Promise.all([
          api.listOrganizations().catch(() => []),
          api.listRemoteProjects().catch(() => []),
        ]);
        setOrgs(orgsList);
        setRemoteProjects(projectsList);
      }
    } catch (err) {
      console.error("Failed to load data", err);
      sendNativeNotification("Error", "Failed to load data");
    } finally {
      setIsFetchingData(false);
    }
  };

  const sendNativeNotification = async (title: string, body: string) => {
    let permissionGranted = await isPermissionGranted();
    if (!permissionGranted) {
      const permission = await requestPermission();
      permissionGranted = permission === "granted";
    }

    if (permissionGranted) {
      sendNotification({ title, body });
    }
  };

  const form = useForm({
    defaultValues: {
      name: "",
      orgId: "",
      localPath: "",
      template: "none",
      projectId: "",
      generateTypescript: true,
      typescriptOutputPath: "",
    },
    validatorAdapter: zodValidator(),
    validators: {
      onSubmit: mode === "create" ? createSchema : syncSchema,
    },
    onSubmit: async ({ value }) => {
      try {
        if (mode === "create") {
          if (isEmptyFolder && value.template !== "none") {
            await api.copyTemplate(value.template, value.localPath.trim());
          }

          await api.createProject(
            value.name.trim(),
            value.localPath.trim(),
            undefined,
            undefined,
            value.orgId,
            value.generateTypescript,
            value.generateTypescript ? value.typescriptOutputPath : undefined
          );
        } else {
          const project = remoteProjects.find((p) => p.id === value.projectId);
          if (!project) throw new Error("Selected project not found");

          await api.createProject(
            project.name,
            value.localPath.trim(),
            undefined,
            project.id,
            undefined,
            value.generateTypescript,
            value.generateTypescript ? value.typescriptOutputPath : undefined
          );
        }

        sendNativeNotification(
          "Success",
          mode === "create" ? "Project created" : "Project synced"
        );
        onCreated();
      } catch (err) {
        console.error(err);
        sendNativeNotification("Error", `Failed to create project: ${err}`);
      }
    },
  });

  const selectFolder = async () => {
    try {
      const selected = await api.pickProjectFolder();
      if (selected) {
        form.setFieldValue("localPath", selected);

        const empty = await api.isFolderEmpty(selected);
        setIsEmptyFolder(empty);

        if (mode === "create" && !form.getFieldValue("name")) {
          const folderName = selected.split("/").pop() || "";
          form.setFieldValue("name", folderName);
        }

        // Default type output path if not set
        if (!form.getFieldValue("typescriptOutputPath")) {
          // We can't easily perform path math here without an invoke to backend or path library
          // But we can just set it to 'src/types/database.ts' relative to the root?
          // Ah, the Requirement says "user needs to choose a folder where the database types file should be added".
          // So we probably want them to pick an ABSOLUTE path for the FOLDER, and we will append database.ts.
          // OR we pick a relative path.
          // Let's assume absolute path for simplicity in UI, but backend might need relative.
          // Wait, `pickProjectFolder` returns an absolute path.
          // If we want a relative path stored, we need to compute it.
          // Let's just default to nothing and make them pick.
        }
      }
    } catch (err) {
      console.error("Failed to select folder:", err);
      sendNativeNotification("Error", "Failed to select folder");
    }
  };

  const selectTypesFolder = async () => {
    try {
      const selected = await api.pickProjectFolder();
      if (selected) {
        // We need to calculate relative path from project root (localPath) to this folder
        // This is hard to do reliably in browser JS without path libs or knowing the separator.
        // However, we can just store the ABSOLUTE path in the UI state for now?
        // Actually, the backend `Project` struct has `typescript_output_path`.
        // If I pass an absolute path to `createProject`, the backend can handle relativeness or just store absolute.
        // The current `Project` struct comment says "relative to project root".
        // I should probably ask the backend to compute the relative path or do it here.
        // For now, let's just pass the string they picked.
        // BUT, the user prompt says "user needs to choose a folder where the database types file should be added".
        // So if they pick `/Users/me/proj/src/types`, we should probably append `database.ts`.

        // Let's assume we pass the full path to the backend and let the backend handle relativeness if it wants,
        // OR we try to handle it here.
        // Given `api` tools, maybe we can just pass the value.

        // Actually, let's just use the selected folder and append /database.ts for display/value?
        // Or just the folder?

        // "user needs to choose a folder where the database types file should be added"
        // So the output path should be that folder + "/database.ts" or similar.

        // Let's just set the value to the selected path.

        // But wait, if we are in a webview, `path` module isn't available.
        // I will just use string manipulation, assuming forward slashes (macOS).

        const projectPath = form.getFieldValue("localPath");
        if (projectPath && selected.startsWith(projectPath)) {
          // Make it relative for cleaner display if possible
          let relative = selected.slice(projectPath.length);
          if (relative.startsWith("/")) relative = relative.slice(1);
          if (relative === "") relative = ".";

          // Append database.ts
          const finalPath = `${relative}/database.ts`;
          form.setFieldValue("typescriptOutputPath", finalPath);
        } else {
          // Just use what they picked (absolute?) or maybe warn it's outside project?
          // Backend might support absolute paths too?
          // The struct comments say "relative", but `sync.rs` handles custom paths.
          // `get_typescript_output_path` joins project path with custom path.
          // So it MUST be relative.

          // If they pick a folder outside, it might be weird.
          // For now, let's assume they pick inside.

          // If they haven't picked a localPath yet, we can't compute relative.
          if (!projectPath) {
            sendNativeNotification(
              "Warning",
              "Please select a Project Folder first."
            );
            return;
          }

          if (!selected.startsWith(projectPath)) {
            sendNativeNotification(
              "Warning",
              "Please select a folder inside the project directory."
            );
            return;
          }

          let relative = selected.slice(projectPath.length);
          if (relative.startsWith("/")) relative = relative.slice(1);
          if (relative === "") relative = ".";
          const finalPath = `${relative}/database.ts`;
          form.setFieldValue("typescriptOutputPath", finalPath);
        }
      }
    } catch (err) {
      console.error("Failed to select folder:", err);
    }
  };

  // Auto-select first org/project when data loads
  useEffect(() => {
    if (orgs.length > 0 && !form.getFieldValue("orgId")) {
      form.setFieldValue("orgId", orgs[0].id);
    }
    if (remoteProjects.length > 0 && !form.getFieldValue("projectId")) {
      form.setFieldValue("projectId", remoteProjects[0].id);
    }
  }, [orgs, remoteProjects, form]);

  return (
    <div className="h-full overflow-y-auto text-sm">
      <div className="flex min-h-full flex-col justify-center items-center py-12">
        <div className="w-full max-w-lg mx-auto">
          <RadioGroup
            value={mode}
            onValueChange={(val) => {
              setMode(val as Mode);
              form.reset();
            }}
            className="grid-cols-2 mb-6"
          >
            {modes.map((m) => (
              <FieldLabel key={m.id} htmlFor={`mode-${m.id}`}>
                <Field orientation="horizontal" className="h-full items-start">
                  <FieldContent>
                    <FieldTitle>{m.title}</FieldTitle>
                    <FieldDescription>{m.description}</FieldDescription>
                  </FieldContent>
                  <RadioGroupItem value={m.id} id={`mode-${m.id}`} />
                </Field>
              </FieldLabel>
            ))}
          </RadioGroup>

          <form
            onSubmit={(e) => {
              e.preventDefault();
              e.stopPropagation();
              form.handleSubmit();
            }}
          >
            <FieldGroup>
              {mode === "create" && (
                <>
                  <form.Field
                    name="orgId"
                    children={(field) => (
                      <Field>
                        <FieldLabel>Organization</FieldLabel>
                        <Select
                          value={field.state.value}
                          onValueChange={field.handleChange}
                          disabled={isFetchingData}
                        >
                          <SelectTrigger>
                            <SelectValue placeholder="Select organization" />
                          </SelectTrigger>
                          <SelectContent>
                            {orgs.map((org) => (
                              <SelectItem key={org.id} value={org.id}>
                                {org.name}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                        {field.state.meta.errors && (
                          <FieldError errors={field.state.meta.errors} />
                        )}
                      </Field>
                    )}
                  />

                  <form.Field
                    name="name"
                    children={(field) => (
                      <Field>
                        <FieldLabel>Project Name</FieldLabel>
                        <Input
                          value={field.state.value}
                          onChange={(e) => field.handleChange(e.target.value)}
                          placeholder="My Supabase Project"
                        />
                        {field.state.meta.errors && (
                          <FieldError errors={field.state.meta.errors} />
                        )}
                      </Field>
                    )}
                  />
                </>
              )}

              {mode === "sync" && (
                <>
                  <form.Field
                    name="orgId"
                    children={(field) => (
                      <Field>
                        <FieldLabel>Organization</FieldLabel>
                        <Select
                          value={field.state.value}
                          onValueChange={(val) => {
                            field.handleChange(val);
                            form.setFieldValue("projectId", ""); // Reset project when org changes
                          }}
                          disabled={isFetchingData}
                        >
                          <SelectTrigger>
                            <SelectValue placeholder="Select organization" />
                          </SelectTrigger>
                          <SelectContent>
                            {orgs.map((org) => (
                              <SelectItem key={org.id} value={org.id}>
                                {org.name}
                              </SelectItem>
                            ))}
                          </SelectContent>
                        </Select>
                        {field.state.meta.errors && (
                          <FieldError errors={field.state.meta.errors} />
                        )}
                      </Field>
                    )}
                  />

                  <form.Field
                    name="projectId"
                    children={(field) => {
                      const selectedOrgId = form.getFieldValue("orgId");
                      const filteredProjects = remoteProjects.filter(
                        (p) => p.organization_id === selectedOrgId
                      );

                      return (
                        <Field>
                          <FieldLabel>Project</FieldLabel>
                          <Select
                            value={field.state.value}
                            onValueChange={field.handleChange}
                            disabled={isFetchingData || !selectedOrgId}
                          >
                            <SelectTrigger>
                              <SelectValue placeholder="Select project" />
                            </SelectTrigger>
                            <SelectContent>
                              {filteredProjects.map((p) => (
                                <SelectItem key={p.id} value={p.id}>
                                  {p.name} ({p.id})
                                </SelectItem>
                              ))}
                            </SelectContent>
                          </Select>
                          {field.state.meta.errors && (
                            <FieldError errors={field.state.meta.errors} />
                          )}
                        </Field>
                      );
                    }}
                  />
                </>
              )}

              <form.Field
                name="localPath"
                children={(field) => (
                  <Field>
                    <FieldLabel>Local Folder</FieldLabel>
                    <div className="flex gap-2">
                      <Input
                        value={field.state.value}
                        readOnly
                        placeholder="Select a folder..."
                        className="flex-1"
                      />
                      <Button
                        type="button"
                        variant="outline"
                        onClick={selectFolder}
                      >
                        Browse
                      </Button>
                    </div>
                    <FieldDescription>
                      The root directory of your project
                    </FieldDescription>
                    {field.state.meta.errors && (
                      <FieldError errors={field.state.meta.errors} />
                    )}
                  </Field>
                )}
              />

              <form.Field
                name="generateTypescript"
                children={(field) => (
                  <Field
                    orientation="horizontal"
                    className="items-center justify-between"
                  >
                    <FieldContent>
                      <FieldLabel className="font-normal" htmlFor="ts-switch">
                        Generate TypeScript Types
                      </FieldLabel>
                      <FieldDescription>
                        Automatically generate TypeScript definitions on schema
                        changes
                      </FieldDescription>
                    </FieldContent>
                    <Switch
                      id="ts-switch"
                      checked={field.state.value}
                      onCheckedChange={field.handleChange}
                    />
                  </Field>
                )}
              />

              <form.Subscribe
                selector={(state) => [state.values.generateTypescript]}
                children={([generateTypescript]) =>
                  generateTypescript ? (
                    <form.Field
                      name="typescriptOutputPath"
                      children={(field) => (
                        <Field>
                          <FieldLabel>Types Output Path</FieldLabel>
                          <div className="flex gap-2">
                            <Input
                              value={field.state.value}
                              onChange={(e) =>
                                field.handleChange(e.target.value)
                              }
                              placeholder="src/types/database.ts"
                              className="flex-1"
                            />
                            <Button
                              type="button"
                              variant="outline"
                              onClick={selectTypesFolder}
                              disabled={!form.getFieldValue("localPath")}
                            >
                              Select Folder
                            </Button>
                          </div>
                          <FieldDescription>
                            Relative path to where database.ts should be
                            generated
                          </FieldDescription>
                          {field.state.meta.errors && (
                            <FieldError errors={field.state.meta.errors} />
                          )}
                        </Field>
                      )}
                    />
                  ) : null
                }
              />

              {mode === "create" && isEmptyFolder && templates.length > 0 && (
                <form.Field
                  name="template"
                  children={(field) => (
                    <Field>
                      <FieldLabel>Template</FieldLabel>
                      <Select
                        value={field.state.value}
                        onValueChange={field.handleChange}
                      >
                        <SelectTrigger>
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
                      <FieldDescription>
                        Choose a starter template for your empty folder
                      </FieldDescription>
                    </Field>
                  )}
                />
              )}

              <div className="flex items-center gap-2 mt-4">
                <Button
                  type="button"
                  variant="outline"
                  onClick={onCancel}
                  className="flex-1"
                >
                  Cancel
                </Button>
                <form.Subscribe
                  selector={(state) => [state.canSubmit, state.isSubmitting]}
                  children={([canSubmit, isSubmitting]) => (
                    <Button
                      type="submit"
                      className="flex-1"
                      disabled={!canSubmit || isSubmitting}
                    >
                      {isSubmitting
                        ? "Saving..."
                        : mode === "create"
                        ? "Create Project"
                        : "Sync Project"}
                    </Button>
                  )}
                />
              </div>
            </FieldGroup>
          </form>
        </div>
      </div>
    </div>
  );
}
