/**
 * The small glob dialect the boundary contract is written in.
 *
 * `*` matches within one path segment, `**` matches across segments. Nothing else is supported,
 * on purpose: a boundary rule that needs a clever pattern is a boundary rule nobody can read.
 */

const escaped = new Set([...".^$+()[]{}|\\"]);

/** Match repo-relative paths, always with `/` separators. */
export function pathMatcher(patterns: readonly string[]): (path: string) => boolean {
  return matcher(patterns.map((pattern) => new RegExp(`^(?:${translate(pattern)})$`)));
}

/**
 * Match module specifiers.
 *
 * A pattern also matches any deeper entry point of what it names, so `pg` covers `pg/lib/pool`
 * and `@temporalio/*` covers `@temporalio/client/internal`. A dependency does not escape a rule
 * by being imported one level down.
 */
export function specifierMatcher(patterns: readonly string[]): (specifier: string) => boolean {
  return matcher(patterns.map((pattern) => new RegExp(`^(?:${translate(pattern)})(?:/.*)?$`)));
}

function matcher(expressions: readonly RegExp[]): (value: string) => boolean {
  return (value) => expressions.some((expression) => expression.test(value));
}

function translate(pattern: string): string {
  const segments = pattern.split("/");
  let body = "";
  segments.forEach((segment, index) => {
    const last = index === segments.length - 1;
    if (segment === "**") {
      body += last ? "(?:.*)?" : "(?:.+/)?";
      return;
    }
    body += segment === "" ? "" : escapeSegment(segment);
    if (!last) body += "/";
  });
  return body;
}

function escapeSegment(segment: string): string {
  let body = "";
  for (const char of segment) {
    if (char === "*") {
      body += "[^/]*";
      continue;
    }
    if (char === "?") {
      body += "[^/]";
      continue;
    }
    body += escaped.has(char) ? `\\${char}` : char;
  }
  return body;
}
