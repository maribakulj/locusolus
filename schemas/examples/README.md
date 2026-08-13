# Fixtures d'exemple LEP v1

Chaque fichier porte un bloc `_fixture` déclarant son `expect`. Ce champ n'appartient pas au
schéma LEP : il est retiré avant validation et sert au corpus de tests (W0.7).

| Paire | `expect` | Ce qu'elle vérifie |
|---|---|---|
| `capability-manifest.json` + `mission-envelope.json` | **refused** | refus d'admission : mission S3, worker macOS Seatbelt S1/S2 |
| `capability-manifest-vm-linux.json` + `mission-envelope-nominal.json` | **accepted** | cas nominal : S3 demandé, S3 offert, toolchain présente |
| `sandbox-attestation.json` | valid | forme d'une attestation |
| `event-reconnection-{1..4}.json` | **replayed** | reconnexion : le rejeu est le *même* document |
| `attempt-late-result.json` + `lease-expired.json` | **quarantined** | résultat rendu après l'expiration de la lease |
| `attempt-budget-exceeded.json` | **budget-exceeded** | arrêt propre sur l'enveloppe, non réessayable |
| `invalid-commit-self-validated.json` | **invalid** | un worker ne peut pas valider son propre commit |
| `invalid-attestation-silent.json` | **invalid** | une attestation muette n'est pas une attestation |
| `invalid-mission-unbounded-budget.json` | **invalid** | une borne libre rend le dépassement inconstatable |

La première paire ne doit **jamais** être lue comme un cas nominal — c'était l'ambiguïté du
package d'origine. Un worker qui l'accepte est en faute.

Les cinq scénarios de W0.7 sont écrits : nominal, refus d'admission, reconnexion, résultat tardif,
dépassement de budget.

## Les trois fixtures `invalid`

Elles ne sont pas des erreurs qu'on aurait oublié de corriger. Un corpus qui ne contient que des
documents valides ne teste que la moitié d'un schéma, et les trois choisies portent les garanties
les plus fortes que W0.5 et W0.6 ont posées. Ce sont elles que le harnais de W0.9 rejouera contre
une implémentation tierce : un serveur qui les accepte est en faute, exactement comme un worker qui
accepterait la paire de refus.

## Le vocabulaire des `expect`

Il vit dans `schemas/registry.json`, avec une note par valeur disant ce qu'elle signifie. Un
résultat nouveau s'ajoute là, documenté ; un `expect` absent du registre fait échouer la
validation plutôt que de passer pour un mot connu.
