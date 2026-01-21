// Spec-driven sidebar types

export interface SidebarSpec {
  groups: Group[];
}

export interface Group {
  id: string;
  name: string;
  icon?: string;

  // How items are populated (choose one)
  items?: Item[]; // Manual items
  itemsQuery?: string; // Dynamic: SQL to get items
  itemTemplate?: Item; // Template for dynamic items (uses :column params)
  itemsFromState?: "tabs"; // From runtime state

  userCreatable?: boolean; // Show + button to create items
}

export interface Item {
  id: string;
  name: string; // Can use :param for dynamic names
  icon?: string;
  visible?: boolean; // default true - if false, hidden child

  type: "query" | "mutation";
  sql: string; // Query SQL or mutation SQL

  // For mutations
  form?: FormConfig;
  loadQuery?: string; // Pre-fill form from query

  // Child items (for navigation)
  children?: Item[];

  // Row actions (for query results)
  rowActions?: RowAction[];

  // Primary action button (above results)
  primaryAction?: {
    label: string;
    itemId: string; // Which child item to navigate to
  };

  autoRun?: boolean; // If true, automatically run the query when opened

  // Chart configuration
  chart?: ChartSpec;
}

export interface ChartSpec {
  xAxis: {
    name: string;
    label?: string;
  };
  yAxis: {
    name: string; // The column to plot
    label?: string;
  }[]; // Allow multiple series
}

export interface FormConfig {
  fields: FormField[];
}

export interface FormField {
  name: string;
  label: string;
  type: "text" | "textarea" | "number" | "boolean" | "select" | "datetime";
  required?: boolean;
  defaultValue?: string | number | boolean;
  placeholder?: string;
  options?: SelectOption[];
  optionsQuery?: string;
}

export interface SelectOption {
  value: string;
  label: string;
}

export interface RowAction {
  label: string;
  variant?: "default" | "destructive";
  itemId: string; // Which child item to navigate to
  params?: Record<string, string>; // Map row columns to item params
}

// Runtime state for a tab's navigation stack
export interface ViewState {
  itemId: string;
  params: Record<string, string>;
}
