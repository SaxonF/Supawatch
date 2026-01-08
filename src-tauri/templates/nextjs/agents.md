# Agent Instructions

This project uses a specific stack and workflow that you should adhere to when making changes or suggesting improvements.

## Tech Stack

- **Styling**: [Tailwind CSS](https://tailwindcss.com/)
- **Components**: [shadcn/ui](https://ui.shadcn.com/)
- **Framework**: Next.js
- **Backend**: Supabase

## Database Workflow

The database schema is managed via a single source of truth:

- **Source of Truth**: `supabase/schemas/schema.sql`
- **Deployment**: Any changes made to `schema.sql` are automatically detected and pushed to the Supabase project.
- **Instruction**: When you need to modify the database (add tables, columns, indexes, policies, etc.), do not run manual SQL commands. Instead, modify the `supabase/schemas/schema.sql` file directly.

## Edge Functions

Edge functions are located in the `supabase/functions` directory:

- **Deployment**: The functions in this directory are automatically deployed whenever changes are detected.
- **Instruction**: To create or update an edge function, simply add or modify the files within the `supabase/functions` folder. No additional deployment steps or commands are required.

## General Guidelines

1. Always prefer using Tailwind utility classes for styling.
2. When creating new UI elements, check if there is an existing shadcn component that can be used or extended.
3. Ensure that any database changes are reflected in `schema.sql` to maintain consistency across environments.
4. For any logic that needs to run on the server or in response to database events, consider using Supabase Edge Functions.
