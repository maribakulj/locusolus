/**
 * Le test de sortie de `W12.f` — le harnais échoue **bruyamment**, il ne se saute pas.
 *
 * # Ce que ce fichier tient, et ce qu'il ne tient pas
 *
 * Il tient la moitié du test de sortie qui est vérifiable **sans les trois processus** : un
 * prérequis absent produit une panne qui le nomme, jamais un saut. C'est la moitié qui compte le
 * plus, parce que c'est celle qui pourrit en silence — un harnais qui se saute rend vert un dossier
 * que personne n'a exercé, et `W20.i` a montré ce que ça coûte.
 *
 * L'autre moitié — les trois processus démarrent et s'arrêtent pour de vrai — est exercée par le job
 * `e2e` de la CI, qui a `podman`, les deux dépôts et les binaires construits. Elle **n'est pas**
 * simulée ici : un test qui monterait de faux processus prouverait que `spawn` fonctionne.
 *
 * Cette séparation est écrite plutôt que subie. `npm test` ne démarre rien ; il vérifie que le
 * harnais refuse correctement, ce qui est une propriété entière et testable seule.
 */

import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, it } from "node:test";

import {
  ATTESTATIONS_ATTENDUES_ENV,
  ATTESTATIONS_ENV,
  EMPREINTE_PREFIXE,
  HarnessFailure,
  WORKER_HOME_ENV,
  WORKER_REPO_ENV,
  annonceDe,
  attestations,
  builtBinary,
  empreinte,
  finDe,
  manifesteDe,
  workerRepo,
} from "./harness.ts";

/** Un dépôt worker crédible : ce que `workerRepo` va chercher, et rien de plus. */
function depotWorker(): string {
  const racine = mkdtempSync(join(tmpdir(), "e2e-repo-"));
  mkdirSync(join(racine, "backend", "cli", "src"), { recursive: true });
  writeFileSync(join(racine, "backend", "cli", "src", "index.ts"), "// worker\n");
  return racine;
}

describe("le harnais e2e refuse plutôt que de se sauter — W12.f", () => {
  it("sans LOCUS_E2E_WORKER, il échoue en nommant la variable", () => {
    assert.throws(
      () => workerRepo({}),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.equal(erreur.subject, WORKER_REPO_ENV);
        // La formule qui compte : le message dit **pourquoi** se sauter serait pire.
        assert.match(erreur.message, /rendrait vert un dossier que personne n'a exercé/);
        return true;
      },
    );
  });

  it("une variable vide vaut une variable absente", () => {
    // Un `LOCUS_E2E_WORKER=""` traîne dans tous les environnements de CI mal remplis. Le lire comme
    // « renseignée » ferait chercher un dépôt à la racine du système, et l'erreur parlerait d'un
    // `index.ts` introuvable plutôt que de la variable — un diagnostic à deux étages pour une
    // cause à un seul.
    for (const valeur of ["", "   "]) {
      assert.throws(
        () => workerRepo({ [WORKER_REPO_ENV]: valeur }),
        (erreur: unknown) => erreur instanceof HarnessFailure && erreur.subject === WORKER_REPO_ENV,
      );
    }
  });

  it("un répertoire qui n'est pas un dépôt canterel est refusé, en disant ce qui manque", () => {
    const vide = mkdtempSync(join(tmpdir(), "e2e-vide-"));

    assert.throws(
      () => workerRepo({ [WORKER_REPO_ENV]: vide }),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.match(erreur.message, /backend\/cli\/src\/index\.ts/);
        return true;
      },
    );
  });

  it("un dépôt worker complet est accepté", () => {
    const racine = depotWorker();

    // Le pendant du test précédent. Une garde qui crierait aussi sur ce qui est juste se ferait
    // désactiver, et c'est le seul moyen de savoir qu'elle regarde la bonne chose.
    assert.equal(workerRepo({ [WORKER_REPO_ENV]: racine }), racine);
  });

  it("un binaire non construit est une panne qui dit comment le construire", () => {
    const racine = mkdtempSync(join(tmpdir(), "e2e-cible-"));

    assert.throws(
      () => builtBinary(racine, "locusd"),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.equal(erreur.subject, "locusd");
        // Un message qui dirait seulement « absent » laisserait chercher la commande.
        assert.match(erreur.message, /cargo build --bin locusd/);
        return true;
      },
    );
  });

  it("un binaire construit est trouvé", () => {
    const racine = mkdtempSync(join(tmpdir(), "e2e-cible-"));
    mkdirSync(join(racine, "target", "debug"), { recursive: true });
    writeFileSync(join(racine, "target", "debug", "locusd"), "");

    assert.equal(builtBinary(racine, "locusd"), join(racine, "target", "debug", "locusd"));
  });

  it("l'échec porte la sortie du processus, pas seulement son nom", () => {
    const erreur = new HarnessFailure(
      "locusd",
      "mort au démarrage (code 1)",
      "adresse déjà utilisée",
    );

    // Sans la sortie, un timeout de démarrage se lit « locusd n'a pas démarré » et se diagnostique
    // en relançant à la main. Avec elle, la cause est dans le message.
    assert.match(erreur.message, /adresse déjà utilisée/);
    assert.match(erreur.message, /--- sortie ---/);
  });

  /**
   * **Le harnais isole le worker par la variable que `canterel` lit réellement.**
   *
   * Un test de constante, et c'est délibéré. La première rédaction posait `XDG_DATA_HOME` — une
   * variable XDG standard, plausible, et que `canterel` **ne lit pas** : `Global.Path.data` dérive
   * de `OPENSCIENCE_TEST_HOME` ou du home réel. Le harnais partageait donc l'installation de la
   * machine en croyant l'isoler, et son verdict dépendait de l'état de cet hôte : vert tant
   * qu'aucun worker n'y était enrôlé, rouge dès qu'il l'était.
   *
   * Rien ne l'aurait montré avant qu'un enrôlement réel réussisse. Le nom est donc figé ici, où un
   * lecteur le voit, plutôt que d'être enfoui dans un appel à `spawn`.
   */
  it("l'isolation passe par la variable que canterel lit, pas par une variable XDG plausible", () => {
    assert.equal(WORKER_HOME_ENV, "OPENSCIENCE_TEST_HOME");
    assert.notEqual(WORKER_HOME_ENV, "XDG_DATA_HOME");
  });

  it("une sortie vide n'ajoute pas de section vide au message", () => {
    const erreur = new HarnessFailure("locusd", "binaire absent", "   ");

    assert.doesNotMatch(erreur.message, /--- sortie ---/);
  });
});

/**
 * L'empreinte d'hôte, lue de ce que le broker annonce — `W5.x`.
 *
 * Testée ici, sans processus, pour la même raison que le reste de ce fichier : la lecture est une
 * propriété entière et exerçable seule. Ce qu'elle ne peut pas tenir seule — qu'un `locus-execd`
 * réel produise bien cette ligne — est l'affaire du job `e2e`, et `chain.chain.ts` l'affirme là-bas.
 */
describe("l'empreinte d'hôte se lit de l'annonce du broker — W5.x", () => {
  /** Ce que `locus-execd` écrit vraiment au démarrage, transcrit de `main.rs`. */
  const ANNONCE = [
    "locus-execd : driver podman",
    "  cgroup v2 : disponible",
    "locus-execd : cet hôte peut prouver S2",
    "  empreinte de cet hôte : cgroup_v2=available controllers=cpu,memory userns=available " +
      "seccomp=available disk_quota=undetermined",
    "locus-execd : à l'écoute sur /tmp/x/broker.sock",
  ].join("\n");

  it("l'empreinte se lit, et rien de ce qui l'entoure n'y entre", () => {
    assert.equal(
      empreinte(ANNONCE),
      "cgroup_v2=available controllers=cpu,memory userns=available seccomp=available " +
        "disk_quota=undetermined",
    );
  });

  /**
   * **Une ligne absente est une panne, pas une empreinte vide.**
   *
   * C'est la garde qui compte. `W5.w` a fait imprimer cette ligne pour que l'exploitant qui prépare
   * un fichier d'attestations sache à quel hôte le lier ; si un remaniement la retire, personne ne
   * le remarque — le refus continue de dire « elles parlent d'un hôte différent » sans jamais dire
   * lequel est celui-ci. Rendre `""` ici ferait exactement disparaître ce constat.
   */
  it("une annonce sans empreinte refuse", () => {
    assert.throws(
      () => empreinte("locus-execd : driver podman\nlocus-execd : à l'écoute sur /tmp/x.sock"),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.match(erreur.message, /cache le remède/);
        return true;
      },
    );
  });

  /**
   * **Une empreinte vide refuse aussi, et pour une autre raison.**
   *
   * Les deux absences ne se réparent pas au même endroit : une ligne manquante est un binaire qui
   * ne dit plus rien, une valeur vide est une empreinte qui ne décide plus de rien — et celle-là
   * serait honorée par n'importe quel hôte, ce qui est pire que de manquer.
   */
  it("une empreinte vide refuse, et ne se confond pas avec une empreinte absente", () => {
    assert.throws(
      () => empreinte("locus-execd : driver podman\n  empreinte de cet hôte :   "),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.match(erreur.message, /honorée partout/);
        return true;
      },
    );
  });

  /**
   * **Le préfixe est celui du binaire, pas une paraphrase.**
   *
   * Un test de constante, comme celui de `WORKER_HOME_ENV` plus haut et pour la même raison : c'est
   * un couplage entre deux langages. Reformuler le `println!` de `main.rs` sans toucher à celle-ci
   * rendrait la lecture muette, et le job `e2e` dirait « empreinte absente » là où elle est écrite
   * juste à côté.
   */
  it("le préfixe lu est celui que main.rs imprime", () => {
    assert.equal(EMPREINTE_PREFIXE, "empreinte de cet hôte :");
  });

  /** Deux processus qui ont parlé, sans en démarrer aucun. */
  const chaine = [
    { name: "locus-execd", output: () => "  empreinte de cet hôte : cgroup_v2=available" },
    { name: "locusd", output: () => "locusd : à l'écoute" },
  ];

  it("l'annonce d'un processus de la chaîne se lit", () => {
    assert.match(annonceDe(chaine, "locus-execd"), /empreinte de cet hôte/);
    assert.equal(annonceDe(chaine, "locusd"), "locusd : à l'écoute");
  });

  /**
   * **Un nom inconnu est une panne, jamais une sortie vide.**
   *
   * C'est la règle « un compteur qui n'a rien lu ne vaut pas zéro », appliquée à une lecture de
   * tampon. Rendre `""` ferait lire `locusexecd` — la faute de frappe la plus banale ici — comme
   * « ce processus n'a rien dit », et l'appelant affirmerait ensuite sur un silence qu'il a
   * fabriqué lui-même. Le message nomme donc les processus qui existent, pour que la faute se
   * corrige sans relire le harnais.
   */
  it("un processus qui n'est pas de la chaîne refuse, et le message nomme ceux qui le sont", () => {
    assert.throws(
      () => annonceDe(chaine, "locusexecd"),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.equal(erreur.subject, "locusexecd");
        assert.match(erreur.message, /locus-execd, locusd/);
        return true;
      },
    );
  });
});

/**
 * Ce que le broker dit du fichier d'attestations — `W5.ab`.
 *
 * `W5.z` a livré la lecture des attestations, `W5.aa` leur dépôt, et **aucun assemblage ne joignait
 * les deux** : le fichier écrit par le job `sandbox` n'était lu par aucun `locus-execd` de CI. La
 * mesure de `W5.x` a levé ce qui l'empêchait — les deux runners rendent la même empreinte,
 * caractère pour caractère.
 */
describe("ce que le broker dit du fichier d'attestations — W5.ab", () => {
  /** L'en-tête que `main` imprime avant toute question d'attestation. */
  const DEBUT = ["locus-execd : driver podman", "locus-execd : cet hôte peut prouver S2"].join(
    "\n",
  );

  it("aucun fichier nommé se lit comme tel", () => {
    const lu = attestations(
      `${DEBUT}\n  attestations : aucune — rien ne sera placé au-dessus de S0`,
    );

    assert.deepEqual(lu, { kind: "aucune" });
  });

  /**
   * **Le cas nominal ne porte pas d'empreinte, et c'est voulu.**
   *
   * `W5.w` l'a décidé : l'empreinte n'apparaît que là où elle sert, c'est-à-dire quand des
   * attestations sont écartées. La lecture doit donc accepter son absence sans la fabriquer.
   */
  it("des attestations honorées se comptent, sans empreinte", () => {
    const lu = attestations(`${DEBUT}\n  attestations : 3 retenue(s) pour cet hôte`);

    assert.deepEqual(lu, { kind: "lues", honorees: 3, etrangeres: 0 });
  });

  it("des attestations écartées se comptent, avec l'empreinte de cet hôte", () => {
    const lu = attestations(
      `${DEBUT}\n  attestations : 0 retenue(s) pour cet hôte, 3 écartée(s) — elles parlent ` +
        "d'un hôte différent de celui-ci, dont l'empreinte est « cgroup_v2=available seccomp=x »",
    );

    assert.deepEqual(lu, {
      kind: "lues",
      honorees: 0,
      etrangeres: 3,
      hote: "cgroup_v2=available seccomp=x",
    });
  });

  /**
   * **Un silence n'est pas « aucune attestation ».**
   *
   * C'est la garde qui compte, et c'est la même règle qu'`annonceDe` plus haut : le lire comme un
   * `S0` nominal enverrait chercher une campagne pendant que le câblage est mort. Les deux états
   * se ressemblent dans un log et ne se réparent pas du tout au même endroit.
   */
  it("un broker muet sur les attestations refuse, il ne vaut pas « aucune »", () => {
    assert.throws(
      () => attestations(DEBUT),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.match(erreur.message, /cessé de rendre compte/);
        return true;
      },
    );
  });

  /** Les deux variables sont celles du produit, pas des paraphrases. */
  it("les variables lues sont celles que locus-execd et le workflow posent", () => {
    assert.equal(ATTESTATIONS_ENV, "LOCUS_EXECD_ATTESTATIONS");
    assert.notEqual(ATTESTATIONS_ENV, "LOCUS_EXECD_ATTESTATION_OUT");
    assert.equal(ATTESTATIONS_ATTENDUES_ENV, "LOCUS_E2E_ATTESTATIONS");
  });
});

describe("le manifeste se lit de ce que le worker imprime — W12.d, quatrième clause", () => {
  const bon = JSON.stringify({
    identity: { worker_id: "canterel-01" },
    manifest: {
      protocol: "lep/1.0",
      worker_id: "canterel-01",
      sandbox: { levels: ["S1", "S2"], network_modes: ["deny", "full"] },
      resources: { cpu_cores: 4, memory_mb: 16_000, disk_free_mb: 7_000 },
    },
  });

  it("rend le manifeste, et lui seul", () => {
    const manifeste = manifesteDe(`quelque chose avant\n${bon}\n`);
    assert.deepEqual([...manifeste.sandbox.levels], ["S1", "S2"]);
    assert.equal(manifeste.resources.cpu_cores, 4);
  });

  /**
   * **« Pas d'identité » n'est pas « pas de manifeste ».**
   *
   * Les deux n'envoient pas au même endroit : le second dit que l'enrôlement du harnais n'a pas
   * pris, et le chercher dans le manifeste ferait perdre un étage. C'est la séparation que ce dépôt
   * applique aux neuf motifs de §12.2, ramenée à une commande.
   */
  it("une installation non enrôlée a son propre refus", () => {
    assert.throws(
      () => manifesteDe("aucune identité : cette installation n'est pas enrôlée\n"),
      (erreur: unknown) => {
        assert.ok(erreur instanceof HarnessFailure);
        assert.match(erreur.message, /enrôl/);
        return true;
      },
    );
  });

  it("une sortie sans JSON, ou un JSON sans manifeste, refusent tous deux", () => {
    for (const sortie of [
      "rien du tout",
      "{ ceci n'est pas du JSON",
      JSON.stringify({ identity: { worker_id: "x" } }),
      // Le cas qui compte : un manifeste **présent** mais amputé des deux champs sur lesquels la
      // mission se taille. Le laisser passer ferait tailler une mission sur `undefined`, et le refus
      // de placement qui suivrait nommerait la mission au lieu de la lecture.
      JSON.stringify({ manifest: { worker_id: "x" } }),
    ]) {
      assert.throws(() => manifesteDe(sortie), HarnessFailure, `« ${sortie} » aurait dû refuser`);
    }
  });
});

describe("un échec de worker rend aussi ce que le daemon a dit — W12.d", () => {
  it("`finDe` garde la fin, qui est ce qui vient d'être dit", () => {
    const sortie = Array.from({ length: 30 }, (_, rang) => `ligne ${rang}`).join("\n");
    const fin = finDe(sortie, 3).split("\n");
    assert.deepEqual(fin, ["ligne 27", "ligne 28", "ligne 29"]);
  });

  it("une sortie plus courte que la borne est rendue entière, sans bourrage", () => {
    assert.equal(finDe("une seule ligne", 12), "une seule ligne");
    assert.equal(finDe("   \n\n  ", 12), "");
  });
});
