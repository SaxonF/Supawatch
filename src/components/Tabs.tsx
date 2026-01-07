import type { Tab } from "../types";
import "./Tabs.css";
import { Button } from "./ui/button";

interface TabsProps {
  activeTab: Tab;
  onTabChange: (tab: Tab) => void;
}

export function Tabs({ activeTab, onTabChange }: TabsProps) {
  return (
    <div className="flex items-center">
      <Button
        variant="ghost"
        size="sm"
        className={`${
          activeTab === "projects" ? "text-primary" : "text-muted-foreground"
        }`}
        onClick={() => onTabChange("projects")}
      >
        Projects
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className={`${
          activeTab === "logs" ? "text-primary" : "text-muted-foreground"
        }`}
        onClick={() => onTabChange("logs")}
      >
        Logs
      </Button>
      <Button
        variant="ghost"
        size="sm"
        className={`${
          activeTab === "settings" ? "text-primary" : "text-muted-foreground"
        }`}
        onClick={() => onTabChange("settings")}
      >
        Settings
      </Button>
    </div>
  );
}
