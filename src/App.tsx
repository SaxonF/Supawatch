import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ask } from "@tauri-apps/plugin-dialog";
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
    const unlistenFileChange = listen<FileChange>("file_change", (event) => {
      console.log("File changed:", event.payload);
    });

    const unlistenConfirmation = listen<{
      project_id: string;
      summary: string;
    }>("schema-push-confirmation-needed", async (event) => {
      const confirmed = await ask(
        `Destructive changes detected during auto-push!\n\n${event.payload.summary}\n\nDo you want to force push these changes?`,
        {
          title: "Destructive Changes Detected",
          kind: "warning",
          okLabel: "Push Changes",
          cancelLabel: "Cancel",
        }
      );

      if (confirmed) {
        try {
          await api.pushProject(event.payload.project_id, true);
          // Optional: Notify user of success via a toast or log
          console.log("Forced push successful");
        } catch (err) {
          console.error("Failed to push project (forced):", err);
          await ask(`Failed to push project: ${err}`, {
            title: "Push Failed",
            kind: "error",
          });
        }
      }
    });

    return () => {
      unlistenFileChange.then((fn) => fn());
      unlistenConfirmation.then((fn) => fn());
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
    <div className="dark h-full border rounded-xl overflow-hidden">
      <div className="bg h-full flex flex-col">
        <header className="shrink-0 flex items-center gap-4 px-5 py-4 border-b justify-between">
          <svg
            width="11"
            height="11"
            viewBox="0 0 11 11"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
          >
            <rect width="1" height="11" fill="white" />
            <rect x="2" width="1" height="11" fill="white" />
            <rect x="10" width="1" height="11" fill="white" />
            <rect x="4" y="6" width="1" height="5" fill="white" />
            <rect x="4" width="1" height="5" fill="white" />
            <rect
              x="9"
              width="1"
              height="5"
              transform="rotate(90 9 0)"
              fill="white"
            />
            <rect
              x="9"
              y="2"
              width="1"
              height="5"
              transform="rotate(90 9 2)"
              fill="white"
            />
            <rect
              x="9"
              y="4"
              width="1"
              height="5"
              transform="rotate(90 9 4)"
              fill="white"
            />
            <rect x="6" y="6" width="1" height="5" fill="white" />
            <rect x="8" y="6" width="1" height="5" fill="white" />
          </svg>

          <Tabs activeTab={activeTab} onTabChange={setActiveTab} />
        </header>

        <main className="flex-1 flex flex-col overflow-hidden">
          {renderContent()}
        </main>
      </div>
    </div>
  );
}

export default App;
