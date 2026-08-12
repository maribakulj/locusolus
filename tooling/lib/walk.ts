import { readdir } from "node:fs/promises";
import { join } from "node:path";

import { pathMatcher } from "./glob.ts";

/**
 * Every file under `root`, repo-relative and sorted, with excluded directories pruned rather
 * than walked.
 *
 * `.git` is always pruned: its contents are history, and history is exactly what the checks in
 * this repository are not allowed to judge.
 */
export async function walkFiles(root: string, exclude: readonly string[]): Promise<string[]> {
  const excluded = pathMatcher(exclude);
  const files: string[] = [];
  const visit = async (directory: string): Promise<void> => {
    const entries = await readdir(join(root, directory), { withFileTypes: true });
    for (const entry of entries) {
      const path = directory ? `${directory}/${entry.name}` : entry.name;
      if (entry.name === ".git" || excluded(path) || excluded(`${path}/`)) continue;
      if (entry.isDirectory()) await visit(path);
      else if (entry.isFile()) files.push(path);
    }
  };
  await visit("");
  return files.sort();
}
