import { cn } from "@/lib/utils";
import {
  ChevronDown,
  ChevronRight,
  FileText,
  Plus,
  RefreshCw,
  Table as TableIcon,
  X,
} from "lucide-react";
import { Button } from "../ui/button";
import { Tab } from "./types";

interface SqlSidebarProps {
  tableTabs: Tab[];
  otherTabs: Tab[];
  activeTabId: string;
  setActiveTabId: (id: string) => void;
  startEditingTab: (id: string) => void;
  editingTabId: string | null;
  editInputRef: React.RefObject<HTMLInputElement | null>;
  editingTabName: string;
  setEditingTabName: (name: string) => void;
  finishEditingTab: () => void;
  handleTabKeyDown: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  closeTab: (id: string, e: React.MouseEvent) => void;
  tablesCollapsed: boolean;
  setTablesCollapsed: (collapsed: boolean) => void;
  fetchTables: () => void;
  isLoadingTables: boolean;
  addNewTab: () => void;
}

export function SqlSidebar({
  tableTabs,
  otherTabs,
  activeTabId,
  setActiveTabId,
  startEditingTab,
  editingTabId,
  editInputRef,
  editingTabName,
  setEditingTabName,
  finishEditingTab,
  handleTabKeyDown,
  closeTab,
  tablesCollapsed,
  setTablesCollapsed,
  fetchTables,
  isLoadingTables,
  addNewTab,
}: SqlSidebarProps) {
  const renderTabItem = (tab: Tab, icon: React.ReactNode) => (
    <div
      key={tab.id}
      onClick={() => setActiveTabId(tab.id)}
      onDoubleClick={() => startEditingTab(tab.id)}
      className={`group flex items-center gap-2 px-3 py-1.5 cursor-pointer transition-colors ${
        tab.id === activeTabId
          ? "bg-primary/10 text-primary border-l-2 border-l-primary"
          : "hover:bg-muted/50 border-l-2 border-l-transparent"
      }`}
    >
      {icon}
      {editingTabId === tab.id ? (
        <input
          ref={editInputRef}
          type="text"
          value={editingTabName}
          onChange={(e) => setEditingTabName(e.target.value)}
          onBlur={finishEditingTab}
          onKeyDown={handleTabKeyDown}
          className="flex-1 bg-transparent border-none outline-none min-w-0"
          onClick={(e) => e.stopPropagation()}
        />
      ) : (
        <span className="flex-1 text truncate" title={tab.name}>
          {tab.name}
        </span>
      )}
      <button
        onClick={(e) => closeTab(tab.id, e)}
        className="shrink-0 opacity-0 group-hover:opacity-100 hover:bg-muted rounded p-0.5 transition-opacity"
        title="Close tab"
      >
        <X size={12} className="text-muted-foreground" />
      </button>
    </div>
  );

  return (
    <div className="w-48 shrink-0 flex flex-col border-r bg">
      {/* Scrollable content */}
      <div className="flex-1 overflow-y-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        {/* Tables Group */}
        <div className="border-b">
          {/* Tables Header - Clickable to collapse */}
          <div
            onClick={() => setTablesCollapsed(!tablesCollapsed)}
            className="shrink-0 flex items-center justify-between px-3 py-2 cursor-pointer hover:bg-muted/30 transition-colors"
          >
            <div className="flex items-center gap-1">
              {tablesCollapsed ? (
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
                Tables
              </span>
            </div>
            <Button
              onClick={(e) => {
                e.stopPropagation();
                fetchTables();
              }}
              variant={"ghost"}
              disabled={isLoadingTables}
              size="icon-sm"
              title="Refresh tables"
            >
              <RefreshCw
                size={14}
                strokeWidth={1}
                className={`text-muted-foreground ${
                  isLoadingTables ? "animate-spin" : ""
                }`}
              />
            </Button>
          </div>

          {/* Tables List */}
          {!tablesCollapsed && (
            <div className="mb-4">
              {tableTabs.map((tab) =>
                renderTabItem(
                  tab,
                  <TableIcon
                    size={14}
                    strokeWidth={1}
                    className={cn("shrink-0 text-muted-foreground", {
                      "text-primary": tab.id === activeTabId,
                    })}
                  />
                )
              )}
              {tableTabs.length === 0 && (
                <div className="px-3 py-2 text-center text-muted-foreground text-xs">
                  No tables found
                </div>
              )}
            </div>
          )}
        </div>

        {/* Other Group */}
        <div>
          {/* Other Header */}
          <div className="shrink-0 flex items-center justify-between px-3 py-2">
            <span className="text-xs font-mono uppercase text-muted-foreground/75 tracking-wider pl-[18px]">
              Other
            </span>
            <Button
              variant={"ghost"}
              size="icon-sm"
              onClick={addNewTab}
              title="New query tab"
            >
              <Plus
                size={14}
                strokeWidth={1}
                className="text-muted-foreground"
              />
            </Button>
          </div>

          {/* Other Tabs List */}
          <div>
            {otherTabs.map((tab) =>
              renderTabItem(
                tab,
                <FileText
                  size={14}
                  strokeWidth={1}
                  className={cn("shrink-0 text-muted-foreground", {
                    "text-primary": tab.id === activeTabId,
                  })}
                />
              )
            )}
            {otherTabs.length === 0 && (
              <div className="px-3 py-2 text-center text-muted-foreground text-xs">
                No queries yet
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
