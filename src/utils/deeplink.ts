/**
 * Deep Link URL Generator
 *
 * Generates harbor:// URLs for sharing sidebar configurations.
 * These URLs can be shared on websites, documentation, etc.
 * When clicked, Harbor will open the Import Template dialog
 * with the URL pre-filled.
 *
 * URL format:
 *   harbor://import?url=<encoded-url>&groupId=<optional-group>
 *
 * The JSON file at the URL should contain one of:
 *   - An Item object (requires groupId in URL or wrapper)
 *   - A Group object
 *   - A full SidebarSpec
 *   - A wrapper: { type: "item", groupId: "admin", item: {...} }
 *   - A wrapper: { type: "group", group: {...} }
 */

const SCHEME = "harbor";

/**
 * Generate a deep link URL that opens the import dialog with a template URL
 *
 * @param templateUrl - URL to the JSON template file
 * @param groupId - Optional group ID for item templates
 * @returns A supawatch:// URL that can be shared
 *
 * @example
 * ```ts
 * // For an item template
 * const url = generateImportLink(
 *   "https://example.com/templates/users-item.json",
 *   "admin"
 * );
 * // Returns: harbor://import?url=https%3A%2F%2Fexample.com%2Ftemplates%2Fusers-item.json&groupId=admin
 *
 * // For a group or full spec template
 * const url = generateImportLink(
 *   "https://example.com/templates/monitoring-group.json"
 * );
 * // Returns: harbor://import?url=https%3A%2F%2Fexample.com%2Ftemplates%2Fmonitoring-group.json
 * ```
 */
export function generateImportLink(
  templateUrl: string,
  groupId?: string,
): string {
  let url = `${SCHEME}://import?url=${encodeURIComponent(templateUrl)}`;
  if (groupId) {
    url += `&groupId=${encodeURIComponent(groupId)}`;
  }
  return url;
}

/**
 * Parse a deep link URL and extract the template URL
 *
 * @param url - The harbor:// URL to parse
 * @returns The parsed info or null if invalid
 */
export function parseDeepLinkUrl(
  url: string,
): { templateUrl: string; groupId?: string } | null {
  try {
    const parsed = new URL(url);
    if (parsed.protocol !== `${SCHEME}:`) {
      return null;
    }

    const action = parsed.hostname;
    const params = parsed.searchParams;

    if (action === "import") {
      const templateUrl = params.get("url");
      if (!templateUrl) {
        return null;
      }
      const groupId = params.get("groupId") || undefined;
      return { templateUrl, groupId };
    }

    return null;
  } catch {
    return null;
  }
}
