import { useEffect, useState } from "react";
import * as api from "../api";
import type { Project } from "../types";
import { CreateProjectForm } from "./CreateProjectForm";
import { ProjectItem } from "./ProjectItem";
import "./ProjectList.css";
import { Button } from "./ui/button";

export function ProjectList() {
  const [projects, setProjects] = useState<Project[]>([]);
  const [showCreateForm, setShowCreateForm] = useState(false);
  const [isLoading, setIsLoading] = useState(true);

  const loadProjects = async () => {
    try {
      const data = await api.getProjects();
      setProjects(data);
    } catch (err) {
      console.error("Failed to load projects:", err);
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    loadProjects();
  }, []);

  const handleProjectCreated = () => {
    setShowCreateForm(false);
    loadProjects();
  };

  if (isLoading) {
    return <div className="loading">Loading projects...</div>;
  }

  return (
    <div className="flex-1 overflow-auto p-5">
      {showCreateForm ? (
        <CreateProjectForm
          onCreated={handleProjectCreated}
          onCancel={() => setShowCreateForm(false)}
        />
      ) : (
        <>
          <Button
            className="add-project-btn mb-4 w-full h-12 rounded-xl text-lg"
            onClick={() => setShowCreateForm(true)}
          >
            + Add Project
          </Button>

          {projects.length === 0 ? (
            <div className="empty-state">
              <p>No projects yet</p>
              <p className="hint">
                Add a project to start watching for changes
              </p>
            </div>
          ) : (
            <div className="space-y-1 rounded-xl overflow-hidden">
              {projects.map((project) => (
                <ProjectItem
                  key={project.id}
                  project={project}
                  onUpdate={loadProjects}
                  onDelete={loadProjects}
                />
              ))}
            </div>
          )}
        </>
      )}
    </div>
  );
}
