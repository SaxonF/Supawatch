import * as api from "@/api";
import { cn } from "@/lib/utils";
import { defaultSidebarSpec, Group, Item } from "@/specs";
import {
  ChevronDown,
  ChevronRight,
  Clock,
  FileText,
  Plus,
  RefreshCw,
  Settings,
  Table as TableIcon,
  X,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import * as store from "../../utils/store";
import { Button } from "../ui/button";
import { Tab } from "./types";

interface SpecSidebarProps {
  projectId: string;
  tabs: Tab[];
  activeTabId: string;
  onTabSelect: (tabId: string) => void;
  onTabCreate: (
    groupId: string,
    item: Item,
    params?: Record<string, string>,
  ) => void;
  onTabClose: (tabId: string, e: React.MouseEvent) => void;
  onTabRename: (tabId: string, name: string) => void;

  // Editing props
  startEditingTab: (id: string) => void;
  editingTabId: string | null;
  editInputRef: React.RefObject<HTMLInputElement | null>;
  editingTabName: string;
  setEditingTabName: (name: string) => void;
  finishEditingTab: () => void;
  handleTabKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
}

// Map icon names to components
const iconMap: Record<
  string,
  React.ComponentType<{
    size?: number;
    strokeWidth?: number;
    className?: string;
  }>
> = {
  Table: TableIcon,
  Clock: Clock,
  Settings: Settings,
  FileText: FileText,
};

interface DynamicItem {
  id: string;
  name: string;
  params: Record<string, string>;
}

export function SpecSidebar({
  projectId,
  tabs,
  activeTabId,
  onTabSelect,
  onTabCreate,
  onTabClose,
  startEditingTab,
  editingTabId,
  editInputRef,
  editingTabName,
  setEditingTabName,
  finishEditingTab,
  handleTabKeyDown,
}: SpecSidebarProps) {
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(
    new Set(),
  );
  const [dynamicItems, setDynamicItems] = useState<
    Record<string, DynamicItem[]>
  >({});
  const [loadingGroups, setLoadingGroups] = useState<Set<string>>(new Set());

  // Load collapsed groups from store
  useEffect(() => {
    // Reset local state first
    setCollapsedGroups(new Set());
    setDynamicItems({});

    const loadState = async () => {
      const persistedCollapsed = await store.load<string[]>(
        store.PROJECT_KEYS.collapsedGroups(projectId),
      );
      if (persistedCollapsed) {
        setCollapsedGroups(new Set(persistedCollapsed));
      }
    };
    loadState();
  }, [projectId]);

  // Toggle group collapse
  const toggleGroup = useCallback(
    (groupId: string) => {
      setCollapsedGroups((prev) => {
        const next = new Set(prev);
        if (next.has(groupId)) {
          next.delete(groupId);
        } else {
          next.add(groupId);
        }
        // Persist change
        store.save(
          store.PROJECT_KEYS.collapsedGroups(projectId),
          Array.from(next),
        );
        return next;
      });
    },
    [projectId],
  );

  // Fetch dynamic items for a group with itemsQuery
  const fetchDynamicItems = useCallback(
    async (group: Group) => {
      if (!group.itemsQuery || !group.itemTemplate) return;

      setLoadingGroups((prev) => new Set(prev).add(group.id));

      try {
        const result = await api.runQuery(projectId, group.itemsQuery, true);

        if (Array.isArray(result)) {
          const items: DynamicItem[] = result.map(
            (row: Record<string, unknown>) => {
              // Build params from row columns
              const params: Record<string, string> = {};
              for (const [key, value] of Object.entries(row)) {
                params[key] = String(value ?? "");
              }

              // Interpolate name from template
              let name = group.itemTemplate!.name;
              for (const [key, value] of Object.entries(params)) {
                name = name.replace(new RegExp(`:${key}`, "g"), value);
              }

              // Interpolate id from template
              let id = group.itemTemplate!.id;
              for (const [key, value] of Object.entries(params)) {
                id = id.replace(new RegExp(`:${key}`, "g"), value);
              }

              return { id, name, params };
            },
          );

          setDynamicItems((prev) => ({ ...prev, [group.id]: items }));
        }
      } catch (err) {
        console.error(`Failed to fetch items for group ${group.id}:`, err);
      } finally {
        setLoadingGroups((prev) => {
          const next = new Set(prev);
          next.delete(group.id);
          return next;
        });
      }
    },
    [projectId],
  );

  // Load dynamic items on mount
  useEffect(() => {
    for (const group of defaultSidebarSpec.groups) {
      if (group.itemsQuery) {
        fetchDynamicItems(group);
      }
    }
  }, [fetchDynamicItems, projectId]); // Re-run when project changes

  // Handle item click - find or create tab
  const handleItemClick = useCallback(
    (groupId: string, item: Item, params: Record<string, string> = {}) => {
      // Check if tab already exists for this item
      const existingTab = tabs.find(
        (t) => t.groupId === groupId && t.specItem?.id === item.id,
      );

      if (existingTab) {
        onTabSelect(existingTab.id);
      } else {
        onTabCreate(groupId, item, params);
      }
    },
    [tabs, onTabSelect, onTabCreate],
  );

  // Render a single item in the sidebar
  const renderItem = (
    groupId: string,
    item: Item,
    isStateDriven: boolean,
    params: Record<string, string> = {},
  ) => {
    if (item.visible === false) return null;

    const IconComponent = item.icon ? iconMap[item.icon] : FileText;
    const existingTab = tabs.find(
      (t) => t.groupId === groupId && t.specItem?.id === item.id,
    );
    const isActive = existingTab?.id === activeTabId;
    const isEditing =
      isStateDriven && existingTab && editingTabId === existingTab.id;

    // Interpolate name
    let displayName = item.name;
    for (const [key, value] of Object.entries(params)) {
      displayName = displayName.replace(new RegExp(`:${key}`, "g"), value);
    }

    return (
      <div
        key={item.id}
        onClick={() => handleItemClick(groupId, item, params)}
        onDoubleClick={() => {
          if (isStateDriven && existingTab) {
            startEditingTab(existingTab.id);
          }
        }}
        className={cn(
          "group flex items-center gap-2 px-3 py-1.5 cursor-pointer transition-colors border-l-2",
          isActive
            ? "bg-primary/10 text-primary border-l-primary"
            : "hover:bg-muted/50 border-l-transparent",
        )}
      >
        {IconComponent && (
          <IconComponent
            size={14}
            strokeWidth={1}
            className={cn("shrink-0 text-muted-foreground", {
              "text-primary": isActive,
            })}
          />
        )}

        {isEditing ? (
          <input
            ref={editInputRef}
            type="text"
            value={editingTabName}
            onChange={(e) => setEditingTabName(e.target.value)}
            onBlur={finishEditingTab}
            onKeyDown={handleTabKeyDown}
            className="flex-1 bg-transparent border-none outline-none min-w-0 text-sm py-0 h-auto"
            onClick={(e) => e.stopPropagation()}
          />
        ) : (
          <span className="flex-1 truncate text-sm" title={displayName}>
            {displayName}
          </span>
        )}

        {existingTab && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onTabClose(existingTab.id, e);
            }}
            className="shrink-0 opacity-0 group-hover:opacity-100 hover:bg-muted rounded p-0.5 transition-opacity"
            title="Close tab"
          >
            <X size={12} className="text-muted-foreground" />
          </button>
        )}
      </div>
    );
  };

  // Render a group
  const renderGroup = (group: Group) => {
    const isCollapsed = collapsedGroups.has(group.id);
    const isLoading = loadingGroups.has(group.id);
    const hasDynamicItems = !!group.itemsQuery;
    const hasUserCreatable = !!group.userCreatable;
    const isStateDriven = group.itemsFromState === "tabs";

    // Get items to render
    let itemsToRender: Array<{ item: Item; params: Record<string, string> }> =
      [];

    if (group.items) {
      // Static items from spec
      itemsToRender = group.items
        .filter((i) => i.visible !== false)
        .map((item) => ({ item, params: {} }));
    } else if (
      group.itemsQuery &&
      group.itemTemplate &&
      dynamicItems[group.id]
    ) {
      // Dynamic items from query
      itemsToRender = dynamicItems[group.id].map((di) => ({
        item: { ...group.itemTemplate!, id: di.id, name: di.name },
        params: di.params,
      }));
    } else if (isStateDriven) {
      // Items from tab state - filter tabs by this group
      const groupTabs = tabs.filter((t) => t.groupId === group.id);
      itemsToRender = groupTabs.map((t) => ({
        item: {
          ...(t.specItem || {
            id: t.id,
            type: "query" as const,
            sql: t.sql,
          }),
          name: t.name,
        },
        params: {},
      }));
    }

    return (
      <div key={group.id} className="border-b">
        {/* Group Header */}
        <div
          onClick={() => toggleGroup(group.id)}
          className="shrink-0 flex items-center h-[42px] justify-between px-3 py-2 cursor-pointer hover:bg-muted/30 transition-colors"
        >
          <div className="flex items-center gap-1">
            {isCollapsed ? (
              <ChevronRight
                size={14}
                strokeWidth={1}
                className="text-muted-foreground"
              />
            ) : (
              <ChevronDown
                size={14}
                strokeWidth={1}
                className="text-muted-foreground"
              />
            )}
            <span className="text-xs font-mono uppercase text-muted-foreground/75 tracking-wider">
              {group.name}
            </span>
          </div>
          <div className="flex items-center gap-1">
            {hasDynamicItems && (
              <Button
                onClick={(e) => {
                  e.stopPropagation();
                  fetchDynamicItems(group);
                }}
                variant="ghost"
                disabled={isLoading}
                size="icon-sm"
                title="Refresh"
              >
                <RefreshCw
                  size={14}
                  strokeWidth={1}
                  className={cn("text-muted-foreground", {
                    "animate-spin": isLoading,
                  })}
                />
              </Button>
            )}
            {hasUserCreatable && group.itemTemplate && (
              <Button
                onClick={(e) => {
                  e.stopPropagation();
                  onTabCreate(group.id, group.itemTemplate!, {});
                }}
                variant="ghost"
                size="icon-sm"
                title="New"
              >
                <Plus
                  size={14}
                  strokeWidth={1}
                  className="text-muted-foreground"
                />
              </Button>
            )}
          </div>
        </div>

        {/* Group Items */}
        {!isCollapsed && (
          <div className="mb-2">
            {itemsToRender.map(({ item, params }) =>
              renderItem(group.id, item, isStateDriven, params),
            )}
            {itemsToRender.length === 0 && !isLoading && (
              <div className="px-8 py-2 text-left text-muted-foreground text-sm">
                No items
              </div>
            )}
            {isLoading && (
              <div className="px-8 py-2 text-left text-muted-foreground text-sm">
                Loading...
              </div>
            )}
          </div>
        )}
      </div>
    );
  };

  return (
    <div className="w-48 shrink-0 flex flex-col border-r bg">
      <div className="flex-1 overflow-y-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        {defaultSidebarSpec.groups.map(renderGroup)}
      </div>
    </div>
  );
}
