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

  // Unified queries support
  queries?: {
    sql: string;

    // Results configuration
    results?: "table" | "chart" | null; // Default 'table'
    chart?: ChartSpec; // Only used if results === 'chart'

    // Input configuration
    parameters?: FormField[]; // Example: if present, show form
    loadQuery?: string; // Query to pre-fill parameters/form values

    // Actions & Navigation
    rowActions?: RowAction[];
    returnToParent?: boolean;
  }[];

  // Primary action button (above results)
  primaryAction?: {
    label: string;
    itemId: string; // Which child item to navigate to
  };

  autoRun?: boolean; // If true, automatically run the query when opened

  // Legacy children for navigation structure (e.g. keeping sidebar hierarchy)
  children?: Item[];
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
