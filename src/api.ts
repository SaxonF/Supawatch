import { invoke } from "@tauri-apps/api/core";
import type { LogEntry, Project, ProjectKeys, RemoteProject } from "./types";

const DEFAULT_RUN_QUERY_TIMEOUT_MS = 60_000;

async function invokeWithTimeout<T>(
  command: string,
  args: Record<string, unknown>,
  timeoutMs: number,
  timeoutMessage: string,
): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    let settled = false;
    const timeoutId = setTimeout(() => {
      settled = true;
      reject(new Error(timeoutMessage));
    }, timeoutMs);

    invoke<T>(command, args)
      .then((result) => {
        if (settled) return;
        settled = true;
        clearTimeout(timeoutId);
        resolve(result);
      })
      .catch((err) => {
        if (settled) return;
        settled = true;
        clearTimeout(timeoutId);
        reject(err);
      });
  });
}

// Access Token API
export async function setAccessToken(token: string): Promise<void> {
  return invoke("set_access_token", { token });
}

export async function hasAccessToken(): Promise<boolean> {
  return invoke("has_access_token");
}

export async function clearAccessToken(): Promise<void> {
  return invoke("clear_access_token");
}

export async function validateAccessToken(): Promise<boolean> {
  return invoke("validate_access_token");
}

// OpenAI API Key
export async function setOpenAiKey(key: string): Promise<void> {
  return invoke("set_openai_key", { key });
}

export async function hasOpenAiKey(): Promise<boolean> {
  return invoke("has_openai_key");
}

export async function clearOpenAiKey(): Promise<void> {
  return invoke("clear_openai_key");
}

// Remote Supabase Projects API
export async function listRemoteProjects(): Promise<RemoteProject[]> {
  return invoke("list_remote_projects");
}

export async function listOrganizations(): Promise<
  import("./types").Organization[]
> {
  return invoke("list_organizations");
}

// Project API
export async function createProject(
  name: string,
  localPath: string,
  supabaseProjectId?: string,
  supabaseProjectRef?: string,
  organizationId?: string,
  generateTypescript: boolean = true,
  typescriptOutputPath?: string,
): Promise<Project> {
  return invoke("create_project", {
    name,
    localPath,
    supabaseProjectId,
    supabaseProjectRef,
    organizationId,
    generateTypescript,
    typescriptOutputPath,
  });
}

export async function getProjects(): Promise<Project[]> {
  return invoke("get_projects");
}

export async function getProject(id: string): Promise<Project> {
  return invoke("get_project", { id });
}

export async function updateProject(project: Project): Promise<Project> {
  return invoke("update_project", { project });
}

export async function deleteProject(id: string): Promise<void> {
  return invoke("delete_project", { id });
}

export async function getProjectKeys(projectId: string): Promise<ProjectKeys> {
  return invoke("get_project_keys", { projectId });
}

export async function revealInFinder(path: string): Promise<void> {
  return invoke("reveal_in_finder", { path });
}

export async function pickProjectFolder(): Promise<string | null> {
  return invoke("pick_project_folder");
}

export async function linkSupabaseProject(
  projectId: string,
  supabaseProjectRef: string,
): Promise<Project> {
  return invoke("link_supabase_project", { projectId, supabaseProjectRef });
}

// Watcher API
export async function startWatching(projectId: string): Promise<void> {
  return invoke("start_watching", { projectId });
}

export async function stopWatching(projectId: string): Promise<void> {
  return invoke("stop_watching", { projectId });
}

export async function isWatching(projectId: string): Promise<boolean> {
  return invoke("is_watching", { projectId });
}

// Logs API
export async function getLogs(
  projectId?: string,
  limit?: number,
): Promise<LogEntry[]> {
  return invoke("get_logs", { projectId, limit });
}

export async function clearLogs(projectId?: string): Promise<void> {
  return invoke("clear_logs", { projectId });
}

// Supabase API
export async function runQuery(
  projectId: string,
  query: string,
  readOnly?: boolean,
  timeoutMs: number = DEFAULT_RUN_QUERY_TIMEOUT_MS,
): Promise<unknown> {
  return invokeWithTimeout(
    "run_query",
    { projectId, query, readOnly },
    timeoutMs,
    `Query timed out after ${Math.ceil(timeoutMs / 1000)} seconds`,
  );
}

export async function validateSql(sql: string): Promise<void> {
  return invoke("validate_sql", { sql });
}

export async function convertWithAi(
  projectId: string,
  input: string,
  errorMessage?: string,
): Promise<string> {
  return invoke("convert_with_ai", {
    projectId,
    input,
    errorMessage,
  });
}

export async function runEdgeFunction(
  projectId: string,
  functionName: string,
  args: Record<string, unknown>,
): Promise<unknown> {
  console.log("args:", args);
  // Get project ref to construct URL
  const project = await getProject(projectId);
  if (!project.supabase_project_ref) {
    throw new Error("Project is not linked to a Supabase project");
  }

  // Get anon key
  const keys = await getProjectKeys(projectId);

  const url = `https://${project.supabase_project_ref}.supabase.co/functions/v1/${functionName}`;

  const { method = "POST", ...bodyArgs } = args;

  console.log("runEdgeFunction", url, method, bodyArgs, keys.anon_key);

  const response = await fetch(url, {
    method: method as string,
    headers: {
      Authorization: `Bearer ${keys.anon_key}`,
      "Content-Type": "application/json",
    },
    body:
      method === "GET" || method === "HEAD"
        ? undefined
        : JSON.stringify(bodyArgs),
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`Edge Function failed: ${response.status} ${text}`);
  }

  const results = await response.json();

  console.log("response", results);

  return results;
}

export async function deployEdgeFunction(
  projectId: string,
  functionSlug: string,
  functionName: string,
  functionPath: string,
): Promise<string> {
  // 3 minute timeout
  const TIMEOUT = 180_000;
  return invokeWithTimeout(
    "deploy_edge_function",
    {
      projectId,
      functionSlug,
      functionName,
      functionPath,
    },
    TIMEOUT,
    "Deployment timed out after 3 minutes",
  );
}

export async function getRemoteSchema(projectId: string): Promise<string> {
  return invoke("get_remote_schema", { projectId });
}

export async function pullProject(projectId: string): Promise<void> {
  return invoke("pull_project", { projectId });
}

export async function pushProject(
  projectId: string,
  force?: boolean,
): Promise<import("./types").PushResponse> {
  // 3 minute timeout (backend http client has 2m timeout)
  const TIMEOUT = 180_000;
  return invokeWithTimeout(
    "push_project",
    { projectId, force },
    TIMEOUT,
    "Push timed out after 3 minutes. Please check your internet connection or try again.",
  );
}

export async function getProjectDiff(
  projectId: string,
): Promise<import("./types").DiffResponse> {
  return invoke("get_project_diff", { projectId });
}

export async function getPullDiff(
  projectId: string,
): Promise<import("./types").PullDiffResponse> {
  return invoke("get_pull_diff", { projectId });
}

export async function splitSchema(projectId: string): Promise<string[]> {
  return invoke("split_schema", { projectId });
}

// Supabase Logs API
export async function querySupabaseLogs(
  projectId: string,
  sql?: string,
  isoTimestampStart?: string,
  isoTimestampEnd?: string,
): Promise<unknown> {
  return invoke("query_supabase_logs", {
    projectId,
    sql,
    isoTimestampStart,
    isoTimestampEnd,
  });
}

export async function getEdgeFunctionLogs(
  projectId: string,
  functionName?: string,
  minutes?: number,
): Promise<unknown> {
  return invoke("get_edge_function_logs", {
    projectId,
    functionName,
    minutes,
  });
}

export async function getPostgresLogs(
  projectId: string,
  minutes?: number,
): Promise<unknown> {
  return invoke("get_postgres_logs", { projectId, minutes });
}

export async function getAuthLogs(
  projectId: string,
  minutes?: number,
): Promise<unknown> {
  return invoke("get_auth_logs", { projectId, minutes });
}

// Templates API
export async function isFolderEmpty(path: string): Promise<boolean> {
  return invoke("is_folder_empty", { path });
}

export async function getTemplates(): Promise<string[]> {
  return invoke("get_templates");
}

export async function copyTemplate(
  templateName: string,
  targetPath: string,
): Promise<void> {
  return invoke("copy_template", { templateName, targetPath });
}

// Seed API
export async function runSeeds(projectId: string): Promise<string> {
  return invoke("run_seeds", { projectId });
}

export async function getSeedContent(projectId: string): Promise<string> {
  return invoke("get_seed_content", { projectId });
}

// Admin Config API
export async function hasAdminConfig(projectId: string): Promise<boolean> {
  return invoke("has_admin_config", { projectId });
}

export async function getSidebarSpec(
  projectId: string,
): Promise<import("./specs/types").SidebarSpec> {
  return invoke("get_sidebar_spec", { projectId });
}

export async function writeSidebarSpec(
  projectId: string,
  spec: import("./specs/types").SidebarSpec,
): Promise<void> {
  return invoke("write_sidebar_spec", { projectId, spec });
}

export async function addSidebarItem(
  projectId: string,
  groupId: string,
  item: import("./specs/types").Item,
): Promise<void> {
  return invoke("add_sidebar_item", { projectId, groupId, item });
}

export async function addSidebarGroup(
  projectId: string,
  group: import("./specs/types").Group,
): Promise<void> {
  return invoke("add_sidebar_group", { projectId, group });
}
