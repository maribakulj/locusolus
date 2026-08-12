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
