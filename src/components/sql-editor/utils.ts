import { ColumnInfo, Tab, TableInfo } from "./types";

// Schema exclusion list matching backend introspection
export const EXCLUDED_SCHEMAS = [
  "pg_catalog",
  "information_schema",
  "auth",
  "storage",
  "extensions",
  "realtime",
  "graphql",
  "graphql_public",
  "vault",
  "pgsodium",
  "pgsodium_masks",
  "supa_audit",
  "net",
  "pgtle",
  "repack",
  "tiger",
  "topology",
  "supabase_migrations",
  "supabase_functions",
  "cron",
  "pgbouncer",
];

// Query to fetch tables matching backend introspection logic
export const TABLES_QUERY = `
  SELECT table_schema as schema, table_name as name
  FROM information_schema.tables
  WHERE table_schema NOT IN (${EXCLUDED_SCHEMAS.map((s) => `'${s}'`).join(
    ", "
  )})
    AND table_schema NOT LIKE 'pg_toast%'
    AND table_schema NOT LIKE 'pg_temp%'
    AND table_type = 'BASE TABLE'
  ORDER BY table_schema, table_name
`;

export function generateTabId(): string {
  return `tab-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`;
}

export function createNewTab(): Tab {
  return {
    id: generateTabId(),
    name: "Untitled",
    sql: "SELECT * FROM ",
    results: [],
    originalResults: [],
    displayColumns: [],
    queryMetadata: null,
    error: null,
    isTableTab: false,
  };
}

// Extract table name, handling quoted identifiers and schema.table format
export function extractTableIdentifier(match: string): string {
  // Remove surrounding quotes if present
  if (match.startsWith('"') && match.endsWith('"')) {
    return match.slice(1, -1);
  }
  return match;
}

// Parse a potentially schema-qualified table reference (schema.table or just table)
export function parseTableReference(ref: string): string {
  // Handle schema.table format - extract just the table name
  const parts = ref.split(".");
  if (parts.length === 2) {
    // Return just the table name part, handling quoted identifiers
    return extractTableIdentifier(parts[1]);
  }
  return extractTableIdentifier(ref);
}

// Regex pattern for table identifiers (quoted or unquoted, with optional schema)
// Matches: table, schema.table, "table", "schema"."table", schema."table"
export const TABLE_IDENTIFIER = `(?:"[^"]+"(?:\\."[^"]+")?|[a-z_][a-z0-9_]*(?:\\.[a-z_][a-z0-9_]*)?(?:\\."[^"]+")?|"[^"]+"\\.[a-z_][a-z0-9_]*)`;
export const SIMPLE_IDENTIFIER = `(?:"[^"]+"|[a-z_][a-z0-9_]*)`;

// Extract the primary table name from a SQL query
export function extractPrimaryTableName(sql: string): string | null {
  const normalized = sql.replace(/\s+/g, " ").trim();
  const fromRegex = new RegExp(`\\bfrom\\s+(${TABLE_IDENTIFIER})`, "i");
  const fromMatch = normalized.match(fromRegex);
  return fromMatch ? parseTableReference(fromMatch[1]) : null;
}

// Parse tables from FROM and JOIN clauses
export function parseTables(sql: string): TableInfo[] {
  const normalized = sql.replace(/\s+/g, " ").trim();
  const tables: TableInfo[] = [];

  // Match FROM table [alias] - supports quoted identifiers and schema.table
  const fromRegex = new RegExp(
    `\\bfrom\\s+(${TABLE_IDENTIFIER})(?:\\s+(?:as\\s+)?(${SIMPLE_IDENTIFIER}))?`,
    "i"
  );
  const fromMatch = normalized.match(fromRegex);
  if (fromMatch) {
    tables.push({
      name: parseTableReference(fromMatch[1]).toLowerCase(),
      alias: fromMatch[2]
        ? extractTableIdentifier(fromMatch[2]).toLowerCase()
        : null,
      primaryKeyColumn: null,
      primaryKeyField: "id",
    });
  }

  // Match JOIN table [alias] - supports quoted identifiers and schema.table
  const joinRegex = new RegExp(
    `\\bjoin\\s+(${TABLE_IDENTIFIER})(?:\\s+(?:as\\s+)?(${SIMPLE_IDENTIFIER}))?`,
    "gi"
  );
  let joinMatch;
  while ((joinMatch = joinRegex.exec(normalized)) !== null) {
    tables.push({
      name: parseTableReference(joinMatch[1]).toLowerCase(),
      alias: joinMatch[2]
        ? extractTableIdentifier(joinMatch[2]).toLowerCase()
        : null,
      primaryKeyColumn: null,
      primaryKeyField: "id",
    });
  }

  return tables;
}

// Check if query has non-editable constructs (aggregations, etc.)
export function hasNonEditableConstructs(sql: string): boolean {
  const normalized = sql.replace(/\s+/g, " ").trim().toLowerCase();

  const nonEditablePatterns = [
    /\bgroup\s+by\b/,
    /\bhaving\b/,
    /\bunion\b/,
    /\bintersect\b/,
    /\bexcept\b/,
    /\bdistinct\b/,
    /\bcount\s*\(/,
    /\bsum\s*\(/,
    /\bavg\s*\(/,
    /\bmin\s*\(/,
    /\bmax\s*\(/,
  ];

  return nonEditablePatterns.some((pattern) => pattern.test(normalized));
}

// Parse column info from SELECT clause and result columns
export function parseColumns(
  sql: string,
  resultColumns: string[],
  tables: TableInfo[]
): ColumnInfo[] {
  const normalized = sql.replace(/\s+/g, " ").trim();

  // Extract SELECT clause
  const selectMatch = normalized.match(/select\s+(.+?)\s+from\s/i);
  if (!selectMatch)
    return resultColumns.map((col) => ({
      resultName: col,
      tableName: null,
      fieldName: col,
      isComputed: false,
      isPrimaryKey: false,
    }));

  const selectClause = selectMatch[1];
  const isSelectStar = selectClause.trim() === "*";

  // Build alias to table name map
  const aliasMap: Record<string, string> = {};
  for (const table of tables) {
    if (table.alias) {
      aliasMap[table.alias] = table.name;
    }
    aliasMap[table.name] = table.name;
  }

  return resultColumns.map((resultCol) => {
    const info: ColumnInfo = {
      resultName: resultCol,
      tableName: null,
      fieldName: resultCol,
      isComputed: false,
      isPrimaryKey: false,
    };

    // Check if column name contains table prefix (e.g., "users.name" or result is "users_name")
    // First, try to find explicit prefix in SELECT clause
    if (!isSelectStar) {
      // Look for patterns like: table.column as alias, table.column alias, table.column
      const prefixPattern = new RegExp(
        `\\b([a-z_][a-z0-9_]*)\\.([a-z_][a-z0-9_]*)(?:\\s+(?:as\\s+)?${resultCol})?\\b`,
        "gi"
      );

      let match;
      while ((match = prefixPattern.exec(selectClause)) !== null) {
        const [fullMatch, prefix, field] = match;
        // Check if this matches our result column
        if (
          fullMatch.toLowerCase().includes(resultCol.toLowerCase()) ||
          field.toLowerCase() === resultCol.toLowerCase()
        ) {
          const tableName = aliasMap[prefix.toLowerCase()];
          if (tableName) {
            info.tableName = tableName;
            info.fieldName = field;
            break;
          }
        }
      }
    }

    // For SELECT *, try to infer table from column naming conventions
    if (!info.tableName && tables.length === 1) {
      // Single table query - all columns belong to that table
      info.tableName = tables[0].name;
    }

    // Check if this is a computed column
    if (!isSelectStar) {
      const computedPatterns = [
        new RegExp(`\\([^)]+\\)\\s+(?:as\\s+)?${resultCol}\\b`, "i"),
        new RegExp(
          `\\w+\\s*[+\\-*/]\\s*\\w+.*?(?:as\\s+)?${resultCol}\\b`,
          "i"
        ),
        new RegExp(`\\w+\\s*\\|\\|\\s*\\w+.*?(?:as\\s+)?${resultCol}\\b`, "i"),
        new RegExp(
          `\\b(?:coalesce|case|nullif|concat)\\s*\\(.*?(?:as\\s+)?${resultCol}\\b`,
          "i"
        ),
      ];

      info.isComputed = computedPatterns.some((p) => p.test(selectClause));
    }

    return info;
  });
}

// Find primary key columns for each table in the result set
export function findPrimaryKeys(
  columns: ColumnInfo[],
  tables: TableInfo[]
): void {
  const pkNames = ["id", "uuid", "pk", "_id"];

  for (const table of tables) {
    // Look for table-prefixed primary key first (e.g., users.id -> users_id or just id if single table)
    for (const pkName of pkNames) {
      const matchingCol = columns.find(
        (col) =>
          col.tableName === table.name && col.fieldName.toLowerCase() === pkName
      );
      if (matchingCol) {
        table.primaryKeyColumn = matchingCol.resultName;
        table.primaryKeyField = pkName;
        matchingCol.isPrimaryKey = true;
        break;
      }
    }

    // If no prefixed PK found, look for standalone pk columns
    if (!table.primaryKeyColumn) {
      for (const pkName of pkNames) {
        const matchingCol = columns.find(
          (col) => col.resultName.toLowerCase() === pkName && !col.isPrimaryKey
        );
        if (matchingCol) {
          table.primaryKeyColumn = matchingCol.resultName;
          table.primaryKeyField = pkName;
          matchingCol.isPrimaryKey = true;
          // Assign this column to the table if not assigned
          if (!matchingCol.tableName) {
            matchingCol.tableName = table.name;
          }
          break;
        }
      }
    }
  }
}

// Format a value for display in the spreadsheet
export function formatCellValue(value: unknown): string {
  if (value === null) return "NULL";
  if (typeof value === "object") {
    return JSON.stringify(value);
  }
  return String(value);
}

// Check if a string looks like JSON
export function isJsonString(str: string): boolean {
  if (str === "NULL") return false;
  const trimmed = str.trim();
  return (
    (trimmed.startsWith("{") && trimmed.endsWith("}")) ||
    (trimmed.startsWith("[") && trimmed.endsWith("]"))
  );
}

// Generate UPDATE SQL for a table
export function generateUpdateSql(
  tableName: string,
  primaryKeyField: string,
  primaryKeyValue: string,
  changes: Record<string, { oldValue: string; newValue: string }>
): string {
  const setClauses = Object.entries(changes)
    .map(([fieldName, { newValue }]) => {
      if (newValue === "NULL") {
        return `"${fieldName}" = NULL`;
      }

      const escapedValue = `'${newValue.replace(/'/g, "''")}'`;

      if (isJsonString(newValue)) {
        return `"${fieldName}" = ${escapedValue}::jsonb`;
      }

      return `"${fieldName}" = ${escapedValue}`;
    })
    .join(", ");

  const escapedPkValue = `'${primaryKeyValue.replace(/'/g, "''")}'`;

  return `UPDATE "${tableName}" SET ${setClauses} WHERE "${primaryKeyField}" = ${escapedPkValue}`;
}
