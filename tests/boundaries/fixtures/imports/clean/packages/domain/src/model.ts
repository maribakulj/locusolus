import type { MissionId } from "@locus/protocol";
import { envelope } from "./envelope.ts";

export const mission = (id: MissionId) => envelope(id);
