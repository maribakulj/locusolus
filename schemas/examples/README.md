# Fixtures d'exemple LEP v1

Chaque fichier porte un bloc `_fixture` déclarant son `expect`. Ce champ n'appartient pas au
schéma LEP : il est retiré avant validation et sert au corpus de tests (W0.7).

| Paire | `expect` | Ce qu'elle vérifie |
|---|---|---|
| `capability-manifest.json` + `mission-envelope.json` | **refused** | refus d'admission : mission S3, worker macOS Seatbelt S1/S2 |
| `capability-manifest-vm-linux.json` + `mission-envelope-nominal.json` | **accepted** | cas nominal : S3 demandé, S3 offert, toolchain présente |
| `sandbox-attestation.json` | valid | forme d'une attestation |

La première paire ne doit **jamais** être lue comme un cas nominal — c'était l'ambiguïté du
package d'origine. Un worker qui l'accepte est en faute.

Restent à écrire en W0.7 : reconnexion, résultat tardif (late candidate), dépassement de budget.
