import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  entryPoints,
  inspectCoherence,
  stringLiterals,
} from "../../tooling/coherence/coherence.ts";
import { readReconciliation } from "../../tooling/repo/roadmap.ts";

const repoRoot = fileURLToPath(new URL("../..", import.meta.url));
const scratch: string[] = [];

after(async () => {
  await Promise.all(scratch.map((path) => rm(path, { recursive: true, force: true })));
});

/** Un dépôt de fixture : un crate, son manifeste, son point d'entrée. */
async function fixture(input: {
  readonly manifest: string;
  readonly main?: string;
}): Promise<string> {
  const root = await mkdtemp(join(tmpdir(), "locus-w22d-"));
  scratch.push(root);
  await mkdir(join(root, "apps/exemple/src"), { recursive: true });
  await writeFile(join(root, "apps/exemple/Cargo.toml"), input.manifest, "utf8");
  if (input.main !== undefined) {
    await writeFile(join(root, "apps/exemple/src/main.rs"), input.main, "utf8");
  }
  return root;
}

const MANIFESTE = [
  "[package]",
  'name = "exemple"',
  "",
  "[[bin]]",
  'name = "exemple"',
  'path = "src/main.rs"',
  "",
].join("\n");

// ---------------------------------------------------------------------------------------------
// 1. Le dépôt lui-même
// ---------------------------------------------------------------------------------------------

/**
 * **Le dépôt lui-même est cohérent, et la garde dit sur quoi elle a conclu.**
 *
 * Les deux binaires du workspace sont lus. Le jour où un troisième entre, il est lu aussi sans que
 * personne y pense — c'est la différence entre une découverte et une liste, et `W22.a` a montré ce
 * que coûte la seconde.
 */
test("le dépôt lui-même est cohérent, sur deux points d'entrée nommés", async () => {
  const { examined, findings } = await inspectCoherence(repoRoot);

  assert.deepEqual(findings, []);
  assert.deepEqual(examined, ["apps/locus-execd/src/main.rs", "apps/locusd/src/main.rs"]);
});

// ---------------------------------------------------------------------------------------------
// 2. Le message d'origine, et celui qu'il ne faut pas confondre avec lui
// ---------------------------------------------------------------------------------------------

/**
 * **Le message qui a menti pendant des mois est attrapé, mot pour mot.**
 *
 * Le test qui porte l'item. C'est le texte exact qu'`apps/locus-execd/src/main.rs` a imprimé pendant
 * que son crate exportait `SystemRunner` — la vérification en rouge se fait donc sur la faute
 * **réelle**, et non sur un exemple fabriqué pour l'occasion.
 */
test("un message qui cite un item du plan est rapporté", async () => {
  const root = await fixture({
    manifest: MANIFESTE,
    main: [
      "fn main() {",
      '    eprintln!("locus-execd : aucun driver de runtime n\'est encore branché (W4.d).");',
      "}",
      "",
    ].join("\n"),
  });

  const { examined, findings } = await inspectCoherence(root);

  assert.equal(examined.length, 1);
  assert.equal(findings.length, 1);
  assert.equal(findings[0]?.rule, "message-qui-cite-le-plan");
  assert.match(findings[0]?.message ?? "", /W4\.d/);
});

/**
 * **Un refus légitime, calculé, n'est pas attrapé.**
 *
 * `locusd` imprime « le port n'est pas ouvert » quand une projection est en quarantaine. C'est une
 * affirmation d'absence, elle est vraie, et elle vieillit avec la machine et non avec le dépôt.
 *
 * Une garde qui aurait cherché la **tournure** — « aucun », « pas », « n'est pas » — l'aurait
 * refusée. C'est pour cela que la règle porte sur un couple déclaration/item et non sur des mots :
 * la première formulation de cet item cherchait « le symbole que le refus déclare manquant », et
 * elle n'aurait attrapé ni celui-ci, ni le vrai défaut, qui ne nomme aucun symbole.
 */
test("un refus calculé, sans citation du plan, passe", async () => {
  const root = await fixture({
    manifest: MANIFESTE,
    main: [
      "fn main() {",
      "    if !readiness.is_ready() {",
      '        eprintln!("locusd : projection en quarantaine. Le port n\'est pas ouvert.");',
      "    }",
      "}",
      "",
    ].join("\n"),
  });

  assert.deepEqual((await inspectCoherence(root)).findings, []);
});

/**
 * **La prose d'un commentaire ne déclenche rien.**
 *
 * Un point d'entrée a le droit d'expliquer dans ses commentaires quel item l'a écrit — c'est même
 * l'usage du dépôt, et `W22.c` le fait. Ce qui est interdit est qu'un identifiant de plan sorte sur
 * la sortie standard d'un programme qui tourne.
 *
 * C'est aussi la garantie que le registre, append-only et plein d'identifiants, ne sera jamais
 * concerné : il n'est pas un point d'entrée, et la garde ne lit que les `[[bin]]` déclarés.
 */
test("un identifiant dans un commentaire ne déclenche pas la garde", async () => {
  const root = await fixture({
    manifest: MANIFESTE,
    main: [
      "//! Le point d'entrée, écrit par W22.c, qui répare ce que W4.d avait laissé dire.",
      "// Voir aussi R3 et W21.j.",
      "fn main() {",
      "    /* W0.17 a réparé la garde de roadmap. */",
      '    println!("prêt");',
      "}",
      "",
    ].join("\n"),
  });

  const { examined, findings } = await inspectCoherence(root);
  assert.equal(examined.length, 1, "le fichier a bien été lu");
  assert.deepEqual(findings, []);
});

/**
 * **Une chaîne brute est lue comme les autres.**
 *
 * L'omettre serait une cécité de plus, et cet item existe parce qu'une cécité ne fait baisser aucun
 * décompte — c'est la leçon de `W22.a`, appliquée à un scanner au lieu d'un motif.
 */
test("une chaîne brute est lue comme les autres", async () => {
  const root = await fixture({
    manifest: MANIFESTE,
    main: ["fn main() {", '    println!(r#"en attente de W20.h"#);', "}", ""].join("\n"),
  });

  const findings = (await inspectCoherence(root)).findings;
  assert.equal(findings.length, 1);
  assert.match(findings[0]?.message ?? "", /W20\.h/);
});

/**
 * **Un item à deux points est cité comme les autres.**
 *
 * C'est la forme que `W22.a` venait de rendre visible à la garde de roadmap. Ne pas la tenir ici
 * laisserait les deux gardes diverger en silence sur ce qu'est un identifiant — et c'est exactement
 * la divergence que `W22.a` a trouvée entre son motif de lignes et son motif d'`attend:`, où
 * `W4.d.1` se lisait `W4.d`, un **autre** item.
 */
test("un item à deux points est cité comme les autres", async () => {
  const root = await fixture({
    manifest: MANIFESTE,
    main: 'fn main() { eprintln!("le driver de W4.d.2 manque"); }\n',
  });

  const findings = (await inspectCoherence(root)).findings;
  assert.equal(findings.length, 1);
  assert.match(findings[0]?.message ?? "", /W4\.d\.2/);
});

/**
 * **Le motif s'arrête aux bornes, et ce qui n'est pas un identifiant n'en est pas un.**
 *
 * Sans bornes, `W5` matcherait à l'intérieur de `W50`, et `SW4.d` — qui ne désigne rien — passerait
 * pour `W4.d`. Une garde qui crie sur ce qui n'existe pas se fait désactiver, et c'est ainsi qu'on
 * perd celles qui avaient raison.
 *
 * La propriété était **écrite** dans le commentaire du motif et n'était **pas testée** : une passe de
 * mutation l'a montré en retirant les bornes sans faire échouer un seul test. C'est le motif que
 * cette phase relève depuis `W21.a` — une propriété décrite sans être testée est une propriété qu'on
 * croit tenir.
 */
test("le motif s'arrête aux bornes", async () => {
  const root = await fixture({
    manifest: MANIFESTE,
    main: [
      "fn main() {",
      "    println!(\"le seuil W50 n'a pas de point, donc pas d'item\");",
      '    println!("SW4.d commence par une lettre, 1R3 par un chiffre, R3X finit par une lettre");',
      "}",
      "",
    ].join("\n"),
  });

  assert.deepEqual((await inspectCoherence(root)).findings, []);
});

/**
 * **Les deux gardes s'accordent sur ce qu'est un identifiant — vérifié contre le plan réel.**
 *
 * Le commentaire du motif annonçait cet accord ; il n'était tenu par rien. La vérification ne
 * compare pas deux expressions régulières — deux copies peuvent être égales et fausses ensemble —
 * mais confronte celle-ci aux identifiants que le **plan** déclare vraiment.
 *
 * Si une forme d'identifiant entre un jour dans la roadmap sans entrer ici, ce test le dit, et la
 * garde de cohérence cesse d'être aveugle à une famille entière sans que rien ne baisse.
 */
test("le motif reconnaît tous les identifiants que le plan déclare", async () => {
  const { planned } = await readReconciliation(repoRoot);
  assert.ok(planned.size > 150, `le plan doit être lu, pas vide : ${planned.size}`);

  const root = await fixture({
    manifest: MANIFESTE,
    main: [...planned]
      .sort()
      .map((item) => `fn f() { println!("${item}"); }`)
      .join("\n"),
  });

  const findings = (await inspectCoherence(root)).findings;
  assert.equal(
    findings.length,
    planned.size,
    "chaque identifiant du plan doit être reconnu comme une citation",
  );
});

// ---------------------------------------------------------------------------------------------
// 3. Le décompte est un verdict
// ---------------------------------------------------------------------------------------------

/**
 * **Aucun point d'entrée examiné est un échec, jamais un « ok ».**
 *
 * C'est la règle de `W22.a` portée à cette garde : un décompte nul ne veut pas dire que tout va
 * bien, il veut dire qu'on n'a rien regardé. Sans elle, retirer les `[[bin]]` des manifestes rendrait
 * la garde muette et verte.
 */
test("aucun point d'entrée examiné fait échouer la garde", async () => {
  const root = await fixture({ manifest: '[package]\nname = "exemple"\n' });

  const { examined, findings } = await inspectCoherence(root);
  assert.deepEqual(examined, []);
  assert.equal(findings.length, 1);
  assert.equal(findings[0]?.rule, "aucun-point-d-entree");
});

/**
 * **Un binaire déclaré dont le fichier manque est rapporté, pas ignoré.**
 *
 * Le manifeste affirme qu'il existe ; ne rien dire reviendrait à conclure sur un fichier qu'on n'a
 * pas lu, et à le compter parmi les examinés serait pire.
 */
test("un binaire déclaré et illisible est rapporté", async () => {
  const root = await fixture({ manifest: MANIFESTE });

  const { examined, findings } = await inspectCoherence(root);
  assert.deepEqual(examined, []);
  assert.equal(
    findings.some((f) => f.rule === "point-d-entree-illisible"),
    true,
  );
});

/**
 * **Les points d'entrée sont découverts, jamais devinés.**
 *
 * Un `[[bin]]` sans `path` n'est pas complété par `src/main.rs` : Cargo le déduirait, mais une garde
 * qui invente son entrée ne dit plus sur quoi elle a conclu. Elle préfère n'avoir rien à examiner —
 * et le dire, ce que la règle précédente transforme en échec.
 */
test("un bin sans chemin n'est pas deviné", async () => {
  const root = await fixture({
    manifest: '[package]\nname = "exemple"\n\n[[bin]]\nname = "exemple"\n',
    main: 'fn main() { println!("W4.d"); }\n',
  });

  assert.deepEqual(await entryPoints(root), []);
});

// ---------------------------------------------------------------------------------------------
// 4. Le scanner de littéraux
// ---------------------------------------------------------------------------------------------

/**
 * **Le scanner rend ce qui est exécutable, et rien d'autre.**
 *
 * Un seul passage plutôt qu'un nettoyage suivi d'une extraction : retirer les commentaires d'abord
 * demanderait de savoir si un `//` est dans une chaîne, c'est-à-dire de faire déjà ce travail.
 */
test("le scanner sépare les chaînes des commentaires", () => {
  const source = [
    '// une chaîne "dans un commentaire" ne compte pas',
    'let a = "une vraie";',
    'let b = "avec un // dedans";',
    'let c = "un guillemet \\" échappé";',
    '/* let d = "dans un bloc"; */',
    'let e = r#"brute avec "guillemets""#;',
  ].join("\n");

  assert.deepEqual(stringLiterals(source), [
    "une vraie",
    "avec un // dedans",
    'un guillemet \\" échappé',
    'brute avec "guillemets"',
  ]);
});

// ---------------------------------------------------------------------------------------------
// 5. Le runner
// ---------------------------------------------------------------------------------------------

function run(root: string): Promise<{ code: number | null; out: string }> {
  const runner = fileURLToPath(
    new URL("../../tooling/coherence/check-coherence.ts", import.meta.url),
  );
  return new Promise((resolve) => {
    execFile("node", [runner, root], (error, stdout, stderr) => {
      const code =
        error && "code" in error && typeof error.code === "number" ? error.code : error ? 1 : 0;
      resolve({ code, out: `${stdout}${stderr}` });
    });
  });
}

/**
 * **Le runner nomme les points d'entrée, pas seulement leur nombre.**
 *
 * « 2 points d'entrée » ne dit pas *lesquels*, et c'est en ne sachant pas lesquels qu'on croit un
 * jour qu'ils sont tous là.
 */
test("le runner nomme ce qu'il a examiné", async () => {
  const propre = await run(repoRoot);
  assert.equal(propre.code, 0);
  assert.match(propre.out, /2 point\(s\) d'entrée examiné\(s\) — apps\/locus-execd/);

  const fautif = await run(
    await fixture({
      manifest: MANIFESTE,
      main: 'fn main() { eprintln!("en attente de W4.d"); }\n',
    }),
  );
  assert.notEqual(fautif.code, 0);
  assert.match(fautif.out, /1 point\(s\) d'entrée examiné\(s\)/);
  assert.match(fautif.out, /message-qui-cite-le-plan/);
});
