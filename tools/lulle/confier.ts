// Accorder la confiance au répertoire de travail d'un worker.
//
// Un worker est sans écran : il ne peut pas répondre à une invite de confiance,
// et sans confiance tous ses appels d'outils sont refusés.
import { Instance } from "@/project/instance"
import { ProjectTrust } from "@/project/trust"

await Instance.provide({
  directory: process.cwd(),
  async fn() {
    const projet = Instance.project
    const avant = await ProjectTrust.status(projet)
    const apres = await ProjectTrust.update(projet, { trusted: true, root: avant.root })
    console.log("%s : %s → %s", apres.root, avant.state, apres.state)
  },
})
