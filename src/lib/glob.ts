/**
 * Convert a single glob pattern to a regex string.
 */
function globToRegex(pattern: string): string {
  let regex = "";
  let i = 0;
  while (i < pattern.length) {
    const ch = pattern[i];
    if (ch === "*") {
      if (pattern[i + 1] === "*") {
        regex += ".*";
        i += 2;
        if (pattern[i] === "/") i++;
      } else {
        regex += "[^/]*";
        i++;
      }
    } else if (ch === "?") {
      regex += "[^/]";
      i++;
    } else if (".(){}[]^$+|\\".includes(ch)) {
      regex += "\\" + ch;
      i++;
    } else {
      regex += ch;
      i++;
    }
  }
  return regex;
}

/**
 * Parse a comma-separated glob pattern string and return a predicate function.
 * Returns null if the input is empty (caller decides default behavior).
 *
 * Supports: `*` (any non-slash), `**` (any including slash), `?` (single char).
 * Patterns without `/` match against any path segment (e.g. `*.ts` matches `src/foo.ts`).
 * Patterns with `/` match against the full path.
 */
export function createGlobMatcher(
  patterns: string,
): ((filepath: string) => boolean) | null {
  const parts = patterns
    .split(",")
    .map((p) => p.trim())
    .filter((p) => p.length > 0);

  if (parts.length === 0) return null;

  const regexParts = parts.map((p) => {
    const regexStr = globToRegex(p);
    if (p.includes("/")) {
      return `^${regexStr}$`;
    }
    return `(?:^|/)${regexStr}$`;
  });

  try {
    const combined = new RegExp(regexParts.join("|"), "i");
    return (filepath: string) => combined.test(filepath);
  } catch {
    return null;
  }
}
