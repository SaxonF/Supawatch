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

const createSchema = z.object({
  name: z.string().min(1, "Project name is required"),
  orgId: z.string().min(1, "Organization is required"),
  localPath: z.string().min(1, "Local path is required"),
  template: z.string(),
  projectId: z.string().optional(),
});

const syncSchema = z.object({
  projectId: z.string().min(1, "Project is required"),
  localPath: z.string().min(1, "Local path is required"),
  orgId: z.string().min(1, "Organization is required"),
  name: z.string().optional(),
  template: z.string().optional(),
});

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
    },
    validatorAdapter: zodValidator(),
    validators: {
      onChange: mode === "create" ? createSchema : syncSchema,
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
            value.orgId
          );
        } else {
          const project = remoteProjects.find((p) => p.id === value.projectId);
          if (!project) throw new Error("Selected project not found");

          await api.createProject(
            project.name,
            value.localPath.trim(),
            undefined,
            project.id,
            undefined
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
      }
    } catch (err) {
      console.error("Failed to select folder:", err);
      sendNativeNotification("Error", "Failed to select folder");
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
                {field.state.meta.errors && (
                  <FieldError errors={field.state.meta.errors} />
                )}
              </Field>
            )}
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
  );
}
