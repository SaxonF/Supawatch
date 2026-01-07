import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import * as api from "./api";
import { LogsViewer } from "./components/LogsViewer";
import { ProjectList } from "./components/ProjectList";
import { Settings } from "./components/Settings";
import { Tabs } from "./components/Tabs";
import type { FileChange, Tab } from "./types";

import "./App.css";

function App() {
  const [activeTab, setActiveTab] = useState<Tab>("projects");
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const initialize = async () => {
      invoke("init");

      // Check if we have an access token, if not show settings
      const hasToken = await api.hasAccessToken();
      if (!hasToken) {
        setActiveTab("settings");
      }
      setIsLoading(false);
    };

    initialize();

    // Listen for file changes to potentially auto-switch to logs
    const unlisten = listen<FileChange>("file_change", (event) => {
      console.log("File changed:", event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const renderContent = () => {
    switch (activeTab) {
      case "projects":
        return <ProjectList />;
      case "logs":
        return <LogsViewer />;
      case "settings":
        return <Settings />;
    }
  };

  if (isLoading) {
    return (
      <div className="app">
        <div className="loading-screen">Loading...</div>
      </div>
    );
  }

  return (
    <div className="bg h-full flex flex-col">
      <header className="shrink-0 flex items-center gap-4 px-5 py-4 bg-muted/50 border-b border-border justify-between">
        <h1 className="font-semibold">Supawatch</h1>
        <Tabs activeTab={activeTab} onTabChange={setActiveTab} />
      </header>

      <main className="flex-1 flex flex-col overflow-hidden">
        {renderContent()}
      </main>
    </div>
  );
}

export default App;
