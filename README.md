# Harbor

Harbor is a macOS menu bar application built with [Tauri](https://tauri.app) that acts as a powerful companion for Supabase development. It synchronizes your local SQL schema definitions with your remote Supabase project, watches for file changes in real-time, and provides tools for managing your entire Supabase workflow.

## Features

- **Schema Synchronization** — Detects changes in your local schema files and pushes them to your Supabase project.
- **Split Schema Files** — Organises your schema into separate files by object type (tables, functions, policies, etc.) for cleaner version control.
- **Introspection** — Reads the current state of your remote database to ensure accurate diffing.
- **Pull & Push** — Pull the remote schema down or push local changes up, with a clear diff preview before applying.
- **Safety Checks** — Warns you before applying destructive changes (like dropping tables or columns).
- **File Watching** — Automatically watches your project directory for changes and keeps everything in sync.
- **Edge Functions** — Deploy Supabase Edge Functions directly from the app.
- **Seed Data** — Run seed SQL files against your database.
- **SQL Editor** — Write and execute SQL queries against your project with an integrated editor.
- **Logs Viewer** — View Postgres, Auth, and Edge Function logs from your Supabase project.
- **Templates** — Import project templates to bootstrap new Supabase projects.
- **Deep Linking** — Supports `harbor://` deep links for integrating with external tools.
- **Native Notifications** — macOS notifications for sync events and deployment results.

## Security & Storage

Harbor securely stores sensitive credentials — your **Supabase Personal Access Token** and **OpenAI API Key** — using the **native macOS Keychain** via the [`keyring`](https://crates.io/crates/keyring) crate with the `apple-native` backend. Secrets never touch the filesystem.

## Architecture & Code Structure

The project is a Tauri v2 app with a **React + TypeScript** frontend (Vite, Tailwind CSS) and a **Rust** backend.

### Backend (`src-tauri/src/`)

The backend is structured around the lifecycle of a schema change:

1.  **Parsing** (`parsing/`) — Reads local SQL files and converts them into structured Rust types (e.g., `TableInfo`, `FunctionInfo`) representing the desired schema state.

2.  **Introspection** (`introspection/`) — Queries the remote Supabase database to build an in-memory representation of the live schema (tables, columns, extensions, policies, triggers, etc.).

3.  **Diff** (`diff/`) — Compares the **Introspected** (remote) state against the **Parsed** (local) state. Determines exactly what needs to change and flags destructive operations.

4.  **Generator** (`generator/`) — Takes the diff output and produces the SQL statements (`CREATE`, `ALTER`, `DROP`) to transition the database to the desired state. Also handles splitting schemas into categorised files.

5.  **Commands** (`commands/`) — Exposes backend capabilities to the frontend via Tauri commands (project management, syncing, deploying, auth, SQL execution, logs, templates, etc.).

6.  **Watcher** (`watcher.rs`) — File system watcher using `notify` that monitors project directories for changes and triggers sync events.

7.  **State** (`state.rs`) — Application state management, including project data, keychain access, schema caching, and watcher lifecycle.

8.  **Supabase API** (`supabase_api.rs`) — HTTP client for the Supabase Management API (schema introspection, edge function deployment, log queries, etc.).

### Frontend (`src/`)

- **Components** (`components/`) — React components for the project list, diff sidebar, pull sidebar, settings, SQL editor, logs viewer, seed runner, and more.
- **API Layer** (`api.ts`) — TypeScript bindings to Tauri commands.
- **Types** (`types.ts`) — Shared TypeScript type definitions.

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/)
- [Node.js](https://nodejs.org/) & [pnpm](https://pnpm.io/)
- [Tauri CLI prerequisites](https://v2.tauri.app/start/prerequisites/)

### Development

1.  **Install dependencies:**

    ```bash
    pnpm i
    ```

2.  **Start the dev server:**

    ```bash
    pnpm tauri dev
    ```

This will compile the Rust backend and launch the app in your menu bar.
