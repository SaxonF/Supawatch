import { useEffect, useState } from "react";
import * as api from "../api";
import type { Project } from "../types";
import { CreateProjectForm } from "./CreateProjectForm";
import { ProjectItem } from "./ProjectItem";
import "./ProjectList.css";

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
          <button
            className="add-project-btn mb-4"
            onClick={() => setShowCreateForm(true)}
          >
            + Add Project
          </button>

          {projects.length === 0 ? (
            <div className="empty-state">
              <p>No projects yet</p>
              <p className="hint">
                Add a project to start watching for changes
              </p>
            </div>
          ) : (
            <div className="projects space-y-2 rounded-xl overflow-hidden">
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
