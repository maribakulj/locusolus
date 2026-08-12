import { extname } from "node:path";

import ts from "typescript";

/**
 * Import extraction, one extractor per language.
 *
 * A language with no extractor is not silently exempt: `analyze.ts` turns it into a
 * `boundary-blind-spot` finding. Adding a language means adding an entry here and moving its
 * extension from the contract's blind list to `extensions.analysable`.
 */

type Extractor = (source: string) => string[];

const extractors = new Map<string, Extractor>([
  [".ts", typescript],
  [".tsx", typescript],
  [".mts", typescript],
  [".cts", typescript],
  [".js", typescript],
  [".jsx", typescript],
  [".mjs", typescript],
  [".cjs", typescript],
  [".el", elisp],
  [".rs", rust],
]);

/** Module specifiers imported by a file, or `null` when its language has no extractor. */
export function specifiersOf(path: string, source: string): string[] | null {
  const extractor = extractors.get(extname(path).toLowerCase());
  return extractor ? extractor(source) : null;
}

export function hasExtractor(path: string): boolean {
  return extractors.has(extname(path).toLowerCase());
}

/**
 * `import`, `export … from`, `require()` and dynamic `import()`, via the TypeScript scanner.
 *
 * Using the real scanner rather than a regular expression is what keeps a specifier inside a
 * comment or a string literal from counting as an import.
 */
function typescript(source: string): string[] {
  return ts.preProcessFile(source, true, true).importedFiles.map((file) => file.fileName);
}

/** `(require 'feature)`, comments stripped. */
function elisp(source: string): string[] {
  const code = source.replace(/;.*$/gm, "");
  return [...code.matchAll(/\(\s*require\s+'([^\s()']+)/g)].flatMap((match) =>
    match[1] ? [match[1]] : [],
  );
}

/**
 * `use` and `extern crate`, with brace groups expanded.
 *
 * Paths are normalised from `::` to `/` so that boundary patterns are written in one dialect for
 * every language: `std::fs::File` becomes `std/fs/File`, which the pattern `std/fs` matches by
 * the same "deeper entry point" rule that lets `pg` match `pg/lib/pool`.
 *
 * A crate whose name carries an underscore is emitted in both forms, because Cargo spells it
 * `tokio-postgres` and Rust code spells the same crate `tokio_postgres`.
 */
function rust(source: string): string[] {
  const code = source.replace(/\/\*[\s\S]*?\*\//g, " ").replace(/\/\/.*$/gm, "");
  const paths = [
    ...[...code.matchAll(/(?:^|[\s{;])(?:pub\s+)?use\s+([^;]+);/g)].flatMap((match) =>
      match[1] ? expandBraces(match[1]) : [],
    ),
    ...[...code.matchAll(/(?:^|[\s{;])extern\s+crate\s+([A-Za-z0-9_]+)/g)].flatMap((match) =>
      match[1] ? [match[1]] : [],
    ),
  ];
  return paths.flatMap(normaliseRustPath);
}

function normaliseRustPath(path: string): string[] {
  const segments = (path.split(" as ")[0] ?? path)
    .split("::")
    .map((segment) => segment.trim())
    .filter((segment) => segment !== "");
  const crate = segments[0];
  if (!crate || crate === "crate" || crate === "self" || crate === "super") return [];
  const joined = segments.join("/");
  if (!crate.includes("_")) return [joined];
  return [joined, [crate.replaceAll("_", "-"), ...segments.slice(1)].join("/")];
}

/** `std::{fs::File, io::Read}` → `std::fs::File`, `std::io::Read`, nesting included. */
function expandBraces(path: string): string[] {
  const open = path.indexOf("{");
  if (open === -1) return [path.trim()];
  const close = closingBrace(path, open);
  if (close === -1) return [path.trim()];
  const prefix = path.slice(0, open);
  const suffix = path.slice(close + 1);
  return splitTopLevel(path.slice(open + 1, close)).flatMap((part) =>
    expandBraces(`${prefix}${part.trim()}${suffix}`),
  );
}

function closingBrace(path: string, open: number): number {
  let depth = 0;
  for (let index = open; index < path.length; index += 1) {
    if (path[index] === "{") depth += 1;
    if (path[index] === "}") {
      depth -= 1;
      if (depth === 0) return index;
    }
  }
  return -1;
}

function splitTopLevel(inner: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let current = "";
  for (const char of inner) {
    if (char === "{") depth += 1;
    if (char === "}") depth -= 1;
    if (char === "," && depth === 0) {
      parts.push(current);
      current = "";
      continue;
    }
    current += char;
  }
  parts.push(current);
  return parts.filter((part) => part.trim() !== "");
}
