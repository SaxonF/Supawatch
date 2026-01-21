// Spec loader
import sidebarSpec from "./sidebar.json";
import type { SidebarSpec } from "./types";

export const defaultSidebarSpec: SidebarSpec =
  sidebarSpec as unknown as SidebarSpec;

export * from "./types";
