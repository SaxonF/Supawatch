# Supawatch

Supawatch is a macOS Menu Bar application built with Tauri that acts as a powerful companion for Supabase development. It synchronizes your local SQL schema definitions with your remote Supabase project in real-time.

## Features

- **Schema Synchronization**: Automatically detects changes in your local `schema.sql` and pushes them to your Supabase project.
- **Introspection**: Reads the current state of your remote database to ensure accurate diffing.
- **Safety Checks**: Warns you before applying destructive changes (like dropping tables or columns).
- **Edge Functions**: Supports deploying Supabase Edge Functions.

## Security & Storage

Supawatch securely stores your Supabase Personal Access Token.

- **Current Implementation**: Tokens are stored in a designated `.token` file within the application data directory (`~/Library/Application Support/supawatch/` on macOS). The content is XOR-obfuscated to prevent being read as plain text. `src/components/Settings.tsx` handles the UI interactions for these tokens.
- **Future Plans**: We are transitioning to using the native macOS Keychain for identifying and managing secrets in Signed builds throughout the future.

## Architecture & Code Structure

The backend logic resides in `src-tauri/src` and is structured around the lifecycle of a schema change:

1.  **Parsing** (`src-tauri/src/parsing/`):

    - Responsible for reading and understanding your local SQL files.
    - It converts raw SQL text into structured Rust structs (e.g., `TableInfo`, `FunctionInfo`) representing your desired schema state.

2.  **Introspection** (`src-tauri/src/introspection/`):

    - Responsible for querying the _current_ state of your remote Supabase database.
    - It fetches metadata about tables, columns, extensions, policies, and more to build an in-memory representation of the live database.

3.  **Diff** (`src-tauri/src/diff/`):

    - The core logic that compares the **Introspected** (remote) state against the **Parsed** (local) state.
    - Calculates exactly what needs to change (e.g., "Add column `email` to table `users`", "Drop policy `public_read`").
    - Determines if changes are destructive.

4.  **Generator** (`src-tauri/src/generator/`):

    - Takes the output from the **Diff** module and generates the actual SQL commands required to update the database.
    - Produces the `CREATE`, `ALTER`, and `DROP` statements to transition the database from its current state to the desired state.

5.  **Commands** (`src-tauri/src/commands/`):
    - Exposes these capabilities to the frontend via Tauri Commands.
    - Handles user interactions such as "creating a project", "syncing", and "deploying functions".

## Getting Started

To run the application locally:

1.  **Install Dependencies**:

    ```bash
    pnpm i
    ```

2.  **Start the Development Server**:
    ```bash
    pnpm tauri dev
    ```

This will launch the app in your menu bar.
