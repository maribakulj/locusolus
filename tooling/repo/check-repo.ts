import { fileURLToPath } from "node:url";

import { report } from "../lib/findings.ts";
import { inspectRepo } from "./layout.ts";

const root = process.argv[2] ?? fileURLToPath(new URL("../..", import.meta.url));

process.exitCode = report("repo-layout", await inspectRepo(root));
