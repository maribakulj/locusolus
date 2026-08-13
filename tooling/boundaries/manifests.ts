/**
 * Dependency names declared in a package manifest.
 *
 * A declared dependency counts as an import. A unit that lists `bollard` in its `Cargo.toml`
 * has already crossed the boundary, whether or not a `use` statement exists yet — and catching
 * it at declaration is catching it at the moment someone decided.
 */

type Reader = (source: string) => string[];

const readers = new Map<string, Reader>([
  ["package.json", npm],
  ["Cargo.toml", cargo],
]);

/** `null` when the file is not a manifest this guard knows how to read. */
export function declaredDependencies(filename: string, source: string): string[] | null {
  const reader = readers.get(filename);
  return reader ? reader(source) : null;
}

export function isManifest(filename: string): boolean {
  return readers.has(filename);
}

function npm(source: string): string[] {
  const fields = ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"];
  try {
    const parsed: unknown = JSON.parse(source);
    if (typeof parsed !== "object" || parsed === null) return [];
    return fields.flatMap((field) => {
      const value = Reflect.get(parsed, field);
      return typeof value === "object" && value !== null ? Object.keys(value) : [];
    });
  } catch {
    return [];
  }
}

/**
 * Dependency names from a `Cargo.toml`.
 *
 * Deliberately not a TOML parser: it reads section headers and left-hand keys, which is all a
 * dependency name ever is. Handles `[dependencies]`, `[dev-dependencies]`, the
 * `[target.'cfg(unix)'.dependencies]` family, `[workspace.dependencies]`, the
 * `[dependencies.name]` sub-table form, and `name.workspace = true`.
 */
function cargo(source: string): string[] {
  const names: string[] = [];
  let section = "";
  for (const raw of source.split("\n")) {
    const line = raw.replace(/#.*$/, "").trim();
    if (line === "") continue;
    const header = /^\[\[?\s*([^\]]+?)\s*\]\]?$/.exec(line);
    if (header?.[1] !== undefined) {
      section = header[1];
      const named = subTableDependency(section);
      if (named) names.push(named);
      continue;
    }
    if (!isDependencySection(section)) continue;
    const key = /^["']?([A-Za-z0-9_.-]+)["']?\s*=/.exec(line);
    const name = key?.[1]?.split(".")[0];
    if (name) names.push(name);
  }
  return names;
}

function isDependencySection(section: string): boolean {
  return /(^|\.)(dev-|build-)?dependencies$/.test(section);
}

function subTableDependency(section: string): string | null {
  const match = /(?:^|\.)(?:dev-|build-)?dependencies\.(.+)$/.exec(section);
  return match?.[1]?.replace(/^["']|["']$/g, "") ?? null;
}
