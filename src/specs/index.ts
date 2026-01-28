// Spec types and default
import type { SidebarSpec } from "./types";

/**
 * Default sidebar spec with only "tables" and "scripts" groups.
 * This matches the backend's DEFAULT_SIDEBAR_SPEC and is used as a fallback
 * when no admin.json exists or while loading.
 */
export const DEFAULT_SIDEBAR_SPEC: SidebarSpec = {
  groups: [
    {
      id: "tables",
      name: "Tables",
      icon: "table",
      itemsSource: {
        type: "sql",
        value:
          "SELECT schemaname AS schema, tablename AS name FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename",
      },
      itemTemplate: {
        id: ":schema.:name",
        icon: "table",
        name: ":name",
        visible: true,
        autoRun: true,
        queries: [
          {
            source: {
              type: "sql",
              value: 'SELECT * FROM ":schema".":name" LIMIT 100',
            },
            results: "table",
          },
        ],
      },
    },
    {
      id: "scripts",
      name: "Scripts",
      icon: "file-text",
      itemsFromState: "tabs",
      userCreatable: true,
      itemTemplate: {
        id: ":id",
        name: "Untitled",
        icon: "file-text",
        visible: true,
        queries: [
          {
            source: {
              type: "sql",
              value: "",
            },
            results: "table",
          },
        ],
      },
    },
  ],
};

export * from "./types";
