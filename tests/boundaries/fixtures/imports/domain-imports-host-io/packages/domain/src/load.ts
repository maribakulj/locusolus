import { readFile } from "node:fs/promises";

export const load = (path: string) => readFile(path, "utf8");
