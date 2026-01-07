import type { Tab } from "../types";
import "./Tabs.css";

interface TabsProps {
  activeTab: Tab;
  onTabChange: (tab: Tab) => void;
}

export function Tabs({ activeTab, onTabChange }: TabsProps) {
  return (
    <div className="flex items-center gap-4">
      <button
        className={`text-muted-foreground ${
          activeTab === "projects" ? "text-primary" : ""
        }`}
        onClick={() => onTabChange("projects")}
      >
        Projects
      </button>
      <button
        className={`text-muted-foreground ${
          activeTab === "logs" ? "text-primary" : ""
        }`}
        onClick={() => onTabChange("logs")}
      >
        Logs
      </button>
      <button
        className={`text-muted-foreground ${
          activeTab === "settings" ? "text-primary" : ""
        }`}
        onClick={() => onTabChange("settings")}
      >
        Settings
      </button>
    </div>
  );
}
