import { Store } from "@tauri-apps/plugin-store";

// We'll use a single store file for all projects, but key data by projectId
const STORE_FILENAME = "supawatch_store.json";

let storeInstance: Store | null = null;

async function getStore() {
  if (!storeInstance) {
    storeInstance = await Store.load(STORE_FILENAME);
  }
  return storeInstance;
}

export async function save(key: string, value: any) {
  const store = await getStore();
  if (key.includes("tabs")) {
    console.error("[PERSISTENCE] Saving tabs:", JSON.stringify(value, null, 2));
  }

  await store.set(key, value);
  await store.save();
}

export async function load<T>(key: string): Promise<T | null> {
  const store = await getStore();
  const value = await store.get<T>(key);

  console.error(
    `[PERSISTENCE] store.load called for key: ${key}. Found value type: ${typeof value}`,
  );

  if (key.includes("tabs")) {
    console.error("[PERSISTENCE] Loaded tabs:", JSON.stringify(value, null, 2));
  }

  return value ?? null;
}

// Project-specific helpers
export const PROJECT_KEYS = {
  activeTab: (projectId: string) => `project:${projectId}:activeTab`,
  tabs: (projectId: string) => `project:${projectId}:tabs`,
  collapsedGroups: (projectId: string) =>
    `project:${projectId}:collapsedGroups`,
};
