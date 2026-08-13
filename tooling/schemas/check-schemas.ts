import { fileURLToPath } from "node:url";

import { report } from "../lib/findings.ts";
import { inspectSchemas } from "./validate.ts";

const root = process.argv.slice(2).find((argument) => !argument.startsWith("--")) ?? defaultRoot();

process.exitCode = report("schemas", inspectSchemas(root));

function defaultRoot(): string {
  return fileURLToPath(new URL("../..", import.meta.url));
}
