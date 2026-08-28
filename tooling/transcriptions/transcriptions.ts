/**
 * Une fixture **transcrite** se vérifie — `W0.22`.
 *
 * # Le défaut qui a motivé cette garde, et il a coûté un tour de CI
 *
 * `tests/e2e/chain.chain.ts` porte une fixture dont l'en-tête dit : « la proposition, **transcrite**
 * de `apps/locusd/tests/commands.rs` et non réinventée […] une fixture propre à ce fichier aurait
 * donné un second corps à maintenir, et un `400` sur un champ oublié se serait lu comme un refus du
 * daemon ».
 *
 * Le raisonnement est juste, et **rien ne vérifiait la transcription**. En livrant `W25.a`, le champ
 * `cognition` est entré côté Rust et pas côté e2e ; `npm run check` est resté vert, parce qu'il ne
 * joue pas l'e2e ; et la CI a rendu exactement le `400 sur un champ oublié` que le commentaire
 * annonçait.
 *
 * C'est la forme de défaut que ce dépôt traque : une propriété affirmée en prose, sans mécanisme
 * derrière. Celle-ci avait en plus décrit d'avance la façon dont elle allait tomber.
 *
 * # Ce que la garde compare, et pourquoi c'est plus qu'un test d'existence
 *
 * Elle confronte l'ensemble des champs d'une structure Rust à l'ensemble des clés d'une fixture
 * JavaScript, **dans les deux sens** :
 *
 * - un champ obligatoire du type qui manque à la fixture est ce qui vient d'arriver ;
 * - une clé de la fixture qui n'existe plus dans le type est le symétrique, et il est plus
 *   discret — `serde` l'ignorerait en silence, donc la fixture décrirait un corps que personne ne
 *   lit, et le test e2e continuerait de passer en exerçant autre chose que ce qu'il annonce.
 *
 * Une garde qui ne dirait que le premier sens serait exacte et à moitié utile.
 *
 * # Ce qu'elle ne fait pas
 *
 * Elle ne compare **pas les valeurs**, ni les types. Deux fixtures peuvent porter les mêmes clés et
 * décrire des questions différentes, et c'est licite : ce que « transcrite » promet est que le corps
 * ait la même **forme**, pas le même contenu. Vérifier plus demanderait d'exécuter les deux, ce qui
 * est le travail de l'e2e lui-même.
 *
 * Elle ne lit pas non plus les types Rust en général : un analyseur de Rust dans une garde de dépôt
 * serait un second compilateur à maintenir. Elle lit une structure **nommée** dans un fichier
 * **nommé**, et elle échoue bruyamment si elle ne la trouve pas.
 */

import { readFile } from "node:fs/promises";
import { join } from "node:path";

import type { Finding } from "../lib/findings.ts";

/** Une transcription déclarée : un type Rust, et la fixture qui le recopie. */
export type Transcription = {
  /** Ce que la garde en dit dans son rapport. */
  readonly nom: string;
  /** Le fichier Rust, relatif à la racine. */
  readonly rust: string;
  /** La structure à y lire. */
  readonly structure: string;
  /** Le fichier JavaScript ou TypeScript, relatif à la racine. */
  readonly fixture: string;
  /** La fonction qui rend l'objet transcrit. */
  readonly fonction: string;
};

/**
 * Les transcriptions du dépôt.
 *
 * **Une seule**, parce qu'il n'y en a qu'une. `CLAUDE.md` refuse la duplication cross-repo des
 * contrats, et celle-ci n'en est pas une : c'est le même corps de requête décrit des deux côtés d'un
 * test de bout en bout, ce qui est exactement ce qu'un e2e fait.
 */
export const TRANSCRIPTIONS: readonly Transcription[] = [
  {
    nom: "la proposition de mission",
    rust: "apps/locusd/src/mission.rs",
    structure: "Proposal",
    fixture: "tests/e2e/chain.chain.ts",
    fonction: "proposition",
  },
];

/** Les champs publics d'une structure Rust nommée. */
export function rustFields(source: string, structure: string): string[] | undefined {
  const debut = source.indexOf(`pub struct ${structure} {`);
  if (debut === -1) return undefined;
  const fin = source.indexOf("\n}", debut);
  if (fin === -1) return undefined;
  const corps = source.slice(debut, fin);
  return [...corps.matchAll(/^\s{4}pub (\w+):/gmu)].map((match) => match[1] ?? "");
}

/**
 * Les clés de premier niveau de l'objet que rend une fonction nommée.
 *
 * # La signature n'est pas regardée, et c'est délibéré — `W20.ac`
 *
 * Cette lecture exigeait `function X() {`, donc zéro paramètre. Le jour où la fixture a dû recevoir
 * l'empreinte de la vue de contexte que la mission nomme, la garde n'a plus rien lu — et elle a
 * refusé bruyamment, comme elle doit. Mais ce qu'elle compare est un **jeu de champs**, pas une
 * signature : une fixture paramétrée décrit le même corps de requête. La lecture part donc de
 * l'accolade ouvrante, quelle que soit la liste d'arguments, et refuse toujours de rendre « ok »
 * quand elle n'a rien trouvé.
 */
export function fixtureKeys(source: string, fonction: string): string[] | undefined {
  const signature = source.indexOf(`function ${fonction}(`);
  if (signature === -1) return undefined;
  // La première accolade après la signature ouvre le corps : aucune liste de paramètres n'en porte.
  const debut = source.indexOf("{", signature);
  if (debut === -1) return undefined;
  const fin = source.indexOf("\n}", debut);
  if (fin === -1) return undefined;
  const corps = source.slice(debut, fin);
  return [...corps.matchAll(/^\s{4}(\w+):/gmu)].map((match) => match[1] ?? "");
}

/** Ce qu'une inspection a lu et trouvé. */
export type Inspection = {
  /** Les transcriptions réellement confrontées — nommées, pas seulement comptées. */
  readonly examined: readonly string[];
  /** Combien de champs ont été comparés. */
  readonly fields: number;
  /** Ce qui ne va pas. */
  readonly findings: readonly Finding[];
};

/** Confronter chaque transcription déclarée à sa source. */
export async function inspectTranscriptions(
  root: string,
  transcriptions: readonly Transcription[] = TRANSCRIPTIONS,
): Promise<Inspection> {
  const findings: Finding[] = [];
  const examined: string[] = [];
  let fields = 0;

  for (const transcription of transcriptions) {
    const { nom, rust, structure, fixture, fonction } = transcription;
    const [sourceRust, sourceFixture] = await Promise.all([
      readFile(join(root, rust), "utf8"),
      readFile(join(root, fixture), "utf8"),
    ]);

    const champs = rustFields(sourceRust, structure);
    const cles = fixtureKeys(sourceFixture, fonction);

    // Rien lu n'est pas rien à dire. Une garde qui ne trouve plus sa structure rendrait « ok » avec
    // la même sérénité qu'une garde qui a tout comparé — c'est la règle 3 du rythme de session, et
    // elle vaut pour l'outillage avant de valoir pour le reste.
    if (champs === undefined || champs.length === 0) {
      findings.push({
        rule: "structure-illisible",
        where: `${rust}`,
        message:
          `« ${structure} » n'y a pas été lue : la transcription ne peut pas être confrontée, et ` +
          "un « ok » ici voudrait dire « je n'ai pas regardé »",
      });
      continue;
    }
    if (cles === undefined || cles.length === 0) {
      findings.push({
        rule: "fixture-illisible",
        where: `${fixture}`,
        message:
          `« ${fonction}() » n'y a pas été lue : la transcription ne peut pas être confrontée, et ` +
          "un « ok » ici voudrait dire « je n'ai pas regardé »",
      });
      continue;
    }

    examined.push(nom);
    fields += champs.length;

    const presentes = new Set(cles);
    for (const champ of champs) {
      if (presentes.has(champ)) continue;
      findings.push({
        rule: "champ-non-transcrit",
        where: fixture,
        message:
          `« ${champ} » existe dans ${structure} et manque à ${fonction}() : le corps envoyé sera ` +
          "refusé, et le refus se lira comme un défaut du daemon plutôt que comme une fixture en retard",
      });
    }

    const connus = new Set(champs);
    for (const cle of cles) {
      if (connus.has(cle)) continue;
      findings.push({
        rule: "champ-inconnu",
        where: fixture,
        message:
          `« ${cle} » est dans ${fonction}() et n'existe pas dans ${structure} : personne ne le lit, ` +
          "donc la fixture décrit un corps qui n'est pas celui qu'on exerce",
      });
    }
  }

  return { examined, fields, findings };
}
