import { fileURLToPath } from "node:url";

import { report } from "../lib/findings.ts";
import { inspectNaming } from "./naming.ts";

const root = process.argv[2] ?? fileURLToPath(new URL("../..", import.meta.url));

process.exitCode = report("naming", await inspectNaming(root));
