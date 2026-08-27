import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { join } from "node:path";

import type { Finding } from "../lib/findings.ts";

/**
 * Le pin du SDK chez le worker, vu depuis le dépôt qui **produit** ce SDK — `W0.24`.
 *
 * # Le trou que cette garde bouche
 *
 * `canterel` embarque une copie épinglée de `packages/lep/src/generated.ts`, et vérifie de son côté
 * qu'elle correspond à sa source — `verifyAgainstSource`, qui rejoue la réécriture déclarée. Cette
 * vérification est **exacte et inopérante en CI** : elle ne tourne que là où une copie de travail de
 * `locusolus` existe, ce dépôt est privé, et la CI du fork ne peut pas le lire. Elle se déclare alors
 * dégradée plutôt que de passer en silence, ce qui est le bon comportement et laisse néanmoins la
 * dérive vivre indéfiniment.
 *
 * Elle a vécu : `W16.d` a changé le SDK généré ici, rien ne l'a redescendu là-bas, et le worker a
 * ignoré `AttemptSubagentsItem`, le champ `subagents` et la feature `subagent-visibility` jusqu'à ce
 * qu'une session travaille les deux dépôts au même endroit.
 *
 * # Pourquoi la garde vit **ici** et pas là-bas
 *
 * Parce que c'est ici que le fichier bouge. Le consommateur ne peut pas savoir qu'une source qu'il
 * n'a pas le droit de lire a changé ; le producteur, lui, le sait toujours. Et le job `e2e` de ce
 * dépôt monte déjà `canterel` à la révision épinglée — la lecture ne coûte donc rien de plus qu'un
 * chemin.
 *
 * # Ce que la garde ne fait pas
 *
 * Elle ne compare pas le fichier **vendu** : c'est l'affaire de `verifyAgainstSource`, qui connaît
 * les réécritures. Elle compare l'empreinte de la **source** telle que le pin l'a enregistrée à
 * celle du fichier tel qu'il est aujourd'hui — la seule question à laquelle ce dépôt peut répondre
 * seul, et la seule qui dise « ta lecture du contrat est périmée ».
 */

/** Le chemin du pin dans le dépôt worker. Une constante, parce que deux dépôts en dépendent. */
export const WORKER_PIN = "backend/cli/src/locus/lep/PINNED.json";

/** Une entrée du pin : un fichier vendu, sa source ici, et les deux empreintes enregistrées. */
export type PinEntry = {
  readonly vendored: string;
  readonly source: string;
  readonly sha256Source: string;
};

/**
 * Ce que la lecture du pin a donné.
 *
 * `illisible` est une **valeur**, pas une exception, et surtout pas un tableau vide. Un pin
 * introuvable ou malformé n'est pas « aucune dérive » : c'est une garde qui n'a rien lu, et la
 * confondre avec un succès est la faute que ce dépôt nomme partout — « un compteur qui n'a rien lu
 * ne vaut pas zéro ».
 */
export type PinReading =
  | { readonly kind: "lu"; readonly commit: string; readonly entries: readonly PinEntry[] }
  | { readonly kind: "illisible"; readonly why: string };

/** Lire le pin d'un dépôt worker monté à `root`. */
export async function readWorkerPin(root: string): Promise<PinReading> {
  const path = join(root, WORKER_PIN);
  const raw = await readFile(path, "utf8").catch((error: NodeJS.ErrnoException) => error);
  if (raw instanceof Error) {
    return { kind: "illisible", why: `${WORKER_PIN} ne se lit pas — ${raw.message}` };
  }

  const parsed: unknown = (() => {
    try {
      return JSON.parse(raw) as unknown;
    } catch (error) {
      return error;
    }
  })();
  if (parsed instanceof Error) {
    return { kind: "illisible", why: `${WORKER_PIN} n'est pas du JSON — ${parsed.message}` };
  }
  if (typeof parsed !== "object" || parsed === null) {
    return { kind: "illisible", why: `${WORKER_PIN} n'est pas un objet` };
  }

  const commit = (parsed as Record<string, unknown>)["commit"];
  const files = (parsed as Record<string, unknown>)["files"];
  if (typeof commit !== "string" || commit === "") {
    return { kind: "illisible", why: `${WORKER_PIN} ne nomme aucun commit` };
  }
  if (typeof files !== "object" || files === null) {
    return { kind: "illisible", why: `${WORKER_PIN} ne porte aucune table \`files\`` };
  }

  const entries: PinEntry[] = [];
  for (const [vendored, value] of Object.entries(files as Record<string, unknown>)) {
    if (typeof value !== "object" || value === null) {
      return {
        kind: "illisible",
        why: `${WORKER_PIN} : l'entrée « ${vendored} » n'est pas un objet`,
      };
    }
    const source = (value as Record<string, unknown>)["source"];
    const sha = (value as Record<string, unknown>)["sha256_source"];
    if (typeof source !== "string" || typeof sha !== "string") {
      return {
        kind: "illisible",
        why: `${WORKER_PIN} : l'entrée « ${vendored} » ne porte pas \`source\` et \`sha256_source\``,
      };
    }
    entries.push({ vendored, source, sha256Source: sha });
  }

  // Une table vide passerait toutes les comparaisons. C'est un pin qui n'épingle rien, donc une
  // garde qui ne garde rien, et le dire vaut mieux que rendre « ok ».
  if (entries.length === 0) {
    return { kind: "illisible", why: `${WORKER_PIN} n'épingle aucun fichier` };
  }
  return { kind: "lu", commit, entries };
}

/**
 * Ce que ce dépôt reproche au pin du worker.
 *
 * Vide quand la lecture du worker est à jour. `root` est la racine de **ce** dépôt : c'est son
 * `packages/lep/**` qui fait foi.
 */
export async function compareWithSource(
  reading: PinReading,
  root: string,
  worker: string,
): Promise<readonly Finding[]> {
  if (reading.kind === "illisible") {
    return [{ rule: "pin-illisible", where: worker, message: reading.why }];
  }

  const findings: Finding[] = [];
  for (const entry of reading.entries) {
    const source = await readFile(join(root, entry.source), "utf8").catch(() => undefined);
    if (source === undefined) {
      // Un renommage **ici** que le consommateur ne peut pas deviner : sa source a disparu sous lui.
      // Le distinguer d'une dérive d'empreinte n'est pas du zèle — l'un se répare en re-vendorant,
      // l'autre en corrigeant la table de `vendor.ts`, et les deux messages n'envoient pas au même
      // endroit.
      findings.push({
        rule: "source-absente",
        where: entry.source,
        message:
          `le pin de « ${worker} » nomme cette source pour « ${entry.vendored} », et elle n'existe ` +
          "plus ici. Un renommage de ce côté ne se voit pas de l'autre : corriger `REWRITES` et le " +
          "pin du consommateur, puis re-vendorer",
      });
      continue;
    }
    const actual = createHash("sha256").update(source, "utf8").digest("hex");
    if (actual !== entry.sha256Source) {
      findings.push({
        rule: "lecture-perimee",
        where: entry.source,
        message:
          `ce fichier a changé depuis que « ${worker} » l'a copié — il épingle ` +
          `${entry.sha256Source.slice(0, 12)}…, il vaut ${actual.slice(0, 12)}…. Le consommateur ` +
          `lit donc un contrat périmé dans « ${entry.vendored} ». Re-vendorer là-bas, puis avancer ` +
          "son pin et `tests/e2e/WORKER-PINNED.json` ici (ADR 0033)",
      });
    }
  }
  return findings;
}
