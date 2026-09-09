;;; locus-mission.el --- Proposer une mission, et la mettre en file  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; `SPEC_V1.md' §15.2 et §15.4.  La moitié qui **commande**.
;;
;; # Pourquoi elle vient après le cockpit, et pas avant
;;
;; `W8.k' a rendu l'état lisible depuis Emacs.  Un laboratoire lisible mais
;; muet est un progrès, pas une station de pilotage : `locusd' sert
;; `/commands/task/propose' et `/commands/task/queue' depuis `W20.h', et rien
;; ne les appelait — la nuée savait s'enrôler et réclamer, et n'avait rien à
;; réclamer.
;;
;; # Ce qu'un humain décide, et tout le reste
;;
;; Une `Proposal' porte seize champs.  Trois seulement sont des décisions :
;; **la question**, **à quoi on reconnaîtra qu'elle est traitée**, et **la
;; classe de cognition**.  Les treize autres sont des bornes de déploiement —
;; confinement, réseau, ressources, budget — qui ne changent pas d'une mission
;; à l'autre sur une machine donnée.
;;
;; Les demander toutes ferait d'une question de recherche un formulaire, et
;; personne ne remplirait le formulaire deux fois.  Elles sont donc des
;; `defcustom' avec des défauts prudents, et la commande ne demande que les
;; trois.
;;
;; # Une classe, jamais un modèle
;;
;; `CognitionClass' vaut `frontier' ou `economy', et le domaine n'accepte aucun
;; identifiant de modèle : quel modèle sert quelle classe est une valeur de
;; politique versionnée, que le worker annonce et que Locus arbitre.  C'est ce
;; qui rend un changement d'affectation gratuit — il ne traverse aucun type, et
;; surtout pas celui-ci.

;;; Code:

(require 'cl-lib)
(require 'locus)
(require 'locus-http)
(require 'locus-session)

(define-error 'locus-mission-refused
  "Le daemon refuse cette mission" 'locus-error)

(defgroup locus-mission nil
  "Proposer des missions depuis Emacs."
  :group 'locus
  :prefix "locus-mission-")

;; --------------------------------------------------------------------------
;; Les bornes, qui ne sont pas des décisions de mission
;; --------------------------------------------------------------------------

(defcustom locus-mission-project nil
  "Le projet auquel les faits de la mission appartiennent, ou nil.

Le daemon refuse une commande sans lui, et le motif dit pourquoi : « sans
projet, un fait n'a pas d'endroit où appartenir ».  C'est une donnée
institutionnelle — c'est l'organisation qui décide où l'on écrit —, donc elle
se configure et ne se devine pas.

Nil et non un identifiant fabriqué : un défaut inventé rangerait les faits de
tout le monde au même endroit, et personne ne s'en apercevrait avant d'aller
les chercher.  `locus-mission-projet' fabrique un projet de session quand
cette variable est nil, et le **dit**."
  :type '(choice (const :tag "aucun — un projet de session sera fabriqué" nil)
                 string)
  :group 'locus-mission)

(defvar locus-mission--projet-de-session nil
  "Le projet fabriqué faute de `locus-mission-project', ou nil.

En mémoire, donc il meurt avec Emacs — ce qui est la bonne durée de vie pour
un journal `personal-local' qui ne survit pas au redémarrage du daemon.")

(defun locus-mission-projet ()
  "Le projet à citer dans les commandes.

`locus-mission-project' quand il est posé.  Sinon un projet de session,
fabriqué une fois et annoncé : un identifiant qui apparaît sans qu'on l'ait
demandé doit au moins se lire quelque part."
  (or locus-mission-project
      locus-mission--projet-de-session
      (let ((projet (locus-mission--id "prj")))
        (setq locus-mission--projet-de-session projet)
        (message "locus : aucun `locus-mission-project' — projet de session %s" projet)
        projet)))

(defcustom locus-mission-sandbox-level "S2"
  "Le plancher de confinement exigé — `S0' à `S5'.

`S2' par défaut : c'est ce que la couche Locus de Canterel exige déjà de
son côté (`minimum_isolation_level: os-sandbox').  Descendre plus bas ferait
proposer des missions qu'aucun worker conforme n'accepterait."
  :type '(choice (const "S0") (const "S1") (const "S2")
                 (const "S3") (const "S4") (const "S5"))
  :group 'locus-mission)

(defcustom locus-mission-network "deny"
  "Le mode réseau imposé à l'attempt.

`deny' par défaut, et c'est la règle du dépôt : « réseau deny-by-default pour
code non fiable ».  Une mission qui a besoin du réseau le dit ; l'inverse
donnerait l'accès à toutes celles qui n'y ont pas pensé."
  :type '(choice (const "deny") (const "connector-only")
                 (const "allowlist") (const "full"))
  :group 'locus-mission)

(defcustom locus-mission-resources
  '((cpu . 1.0) (memory_mb . 2048) (disk_mb . 4096) (wall_time_seconds . 900))
  "Les ressources réservées — invariant 6, « réservées avant exécution ».

Pas d'accélérateur : son absence veut dire « aucun n'est requis », jamais
« n'importe lequel fera l'affaire » (invariant 8)."
  :type '(alist :key-type symbol :value-type sexp)
  :group 'locus-mission)

(defcustom locus-mission-budget
  '((max_model_calls . 40) (max_input_tokens . 200000) (max_output_tokens . 40000))
  "Les trois bornes de modèle, toutes obligatoires.

Un budget est une borne, pas une prévision : le dépasser arrête l'attempt.
Les valeurs par défaut tiennent une session de travail ordinaire et se
remontent quand une mission le mérite — ce qui est une décision qu'on prend
en la prenant, pas un défaut qu'on subit."
  :type '(alist :key-type symbol :value-type integer)
  :group 'locus-mission)

(defcustom locus-mission-output-contract "markdown"
  "Ce que l'attempt doit rendre."
  :type 'string
  :group 'locus-mission)

(defcustom locus-mission-clearance "internal"
  "Le plafond de confidentialité du destinataire de la vue de contexte.

`internal' et pas plus haut : au-delà, un modèle dont les prompts quittent la
machine est exclu par la politique, et la mission ne trouverait aucun worker
sur un fournisseur distant.  Le relever est un choix qui restreint la nuée,
donc il se fait sciemment."
  :type '(choice (const "public") (const "internal")
                 (const "confidential") (const "restricted"))
  :group 'locus-mission)

;; --------------------------------------------------------------------------
;; Les identifiants
;; --------------------------------------------------------------------------

(defconst locus-mission--alphabet "0123456789ABCDEFGHJKMNPQRSTVWXYZ"
  "Base 32 de Crockford — l'alphabet de `packages/protocol'.

`I', `L', `O' et `U' en sont absents, parce qu'ils se lisent pour autre chose.
Un générateur qui les produirait fabriquerait des identifiants refusés à la
lecture, et le refus arriverait loin d'ici.")

(defun locus-mission--id (prefixe)
  "Un identifiant Locus sous PREFIXE.

Vingt-six caractères, dont le premier vaut au plus 7 : 26 × 5 bits font 130
pour une valeur de 128, donc les deux bits de tête n'existent pas."
  (concat prefixe "_"
          (string (aref locus-mission--alphabet (random 8)))
          (mapconcat (lambda (_)
                       (string (aref locus-mission--alphabet (random 32))))
                     (number-sequence 1 25) "")))

;; --------------------------------------------------------------------------
;; Parler au daemon
;; --------------------------------------------------------------------------

(defun locus-mission--post (path body)
  "Envoyer BODY en POST sur PATH, et rendre le corps relu.

Une clé d'idempotence est posée par l'appelant, dans BODY : §22.5 la veut sur
le fil, et la fabriquer ici la rendrait différente à chaque tentative — ce qui
est exactement ce qu'une clé d'idempotence existe pour empêcher.

# Errors

`locus-mission-refused' quand le daemon refuse.  L'enveloppe structurée du
serveur est rendue telle quelle : elle porte la catégorie et la politique de
reprise, là où le statut seul obligerait à deviner."
  (pcase-let* ((`(,host . ,port) (locus-session-host-port))
               (request (locus-http-build
                         "POST" (concat "/" path)
                         :headers (list (cons "Host" (format "%s:%d" host port))
                                        (cons "Connection" "close"))
                         :body body))
               (response
                (condition-case err
                    (locus-http-send host port (locus-session--authorized request))
                  (error (signal 'locus-mission-refused
                                 (list (format "%s : %s" path
                                               (locus-session--cause err))))))))
    (let ((status (alist-get :status response)))
      (unless (and status (>= status 200) (< status 300))
        (signal 'locus-mission-refused
                (list (format "%s : le daemon répond %s%s" path status
                              (let ((envelope (alist-get :error response)))
                                (if envelope (format " — %S" envelope)
                                  (let ((corps (alist-get :body response)))
                                    (if corps (format " — %S" corps) ""))))))))
      (alist-get :body response))))

(defun locus-mission--vue-de-contexte (question)
  "Construire une vue de contexte pour QUESTION, et rendre (ID . HASH).

# Une vue vide est une vue, et elle est honnête

Aucun candidat n'est soumis : sur un laboratoire qui démarre, le graphe ne
porte rien qu'une mission puisse citer.  La vue dit donc « voici la question,
et tu n'as accès à rien » — ce qui est vrai, vérifiable, et scellé par un
hash comme n'importe quelle autre.

L'alternative serait de sauter la vue, et elle n'existe pas : la mission ne
porte jamais son contexte, seulement sa référence et son empreinte, et c'est
cette empreinte qui rend « ce que l'agent pouvait connaître » vérifiable
après coup."
  (let* ((id (locus-mission--id "ctx"))
         (rendu (locus-mission--post
                 "commands/context-view/build"
                 `((idempotency_key . ,(locus-mission--id "cmd"))
                   (project_id . ,(locus-mission-projet))
                   (view . ((id . ,id)
                            (query . ,question)
                            (source_event_watermark . 0)
                            (recipient . ((agent_id . ,(locus-mission--id "agent"))
                                          (worker_id . "any")
                                          (blind_to_generator . :false)
                                          (clearance . ,locus-mission-clearance)))
                            (candidates . []))))))
         (vue (or (alist-get 'view rendu) rendu)))
    (cons (or (alist-get 'id vue) id)
          (or (alist-get 'hash vue)
              (alist-get 'view_hash vue)
              (alist-get 'content_hash vue)))))

;;;###autoload
(defun locus-mission-propose (question conditions cognition)
  "Proposer une mission : QUESTION, ses CONDITIONS de succès, sa COGNITION.

Trois questions, parce que trois décisions.  Le reste vient des `defcustom'
de ce module, qui sont des bornes de déploiement et non des choix de mission.

Rend l'identifiant de la tâche ouverte — c'est lui que
`locus-mission-queue' met en file."
  (interactive
   (list (read-string "La question : ")
         (split-string
          (read-string "À quoi on la reconnaîtra traitée (séparé par « ; ») : ")
          "[ \t]*;[ \t]*" t)
         (completing-read "Classe de cognition : " '("frontier" "economy") nil t
                          nil nil "frontier")))
  (when (string-empty-p (string-trim question))
    (user-error "Une mission sans question n'en est pas une"))
  (when (null conditions)
    (user-error "Sans condition de succès, rien ne dira que la mission est traitée"))
  (pcase-let* ((`(,vue-id . ,vue-hash) (locus-mission--vue-de-contexte question))
               (task-id (locus-mission--id "task"))
               (rendu
                (locus-mission--post
                 "commands/task/propose"
                 `((idempotency_key . ,(locus-mission--id "cmd"))
                   (project_id . ,(locus-mission-projet))
                   (proposal
                    . ((cognition . ,cognition)
                       (statement . ,question)
                       (success_conditions . ,(vconcat conditions))
                       (task_id . ,task-id)
                       (attempt_id . ,(locus-mission--id "att"))
                       ;; Le rang est fixé par la proposition, jamais compté par
                       ;; le daemon : « une tâche réattribuée conserve son
                       ;; numéro d'attempt », donc un compteur de réclamations
                       ;; donnerait un rang neuf à une reprise après panne.
                       (attempt . 1)
                       (branch_id . ,(locus-mission--id "br"))
                       (context_view_id . ,vue-id)
                       (context_view_hash . ,vue-hash)
                       (environment_id . ,(locus-mission--id "env"))
                       (sandbox_level . ,locus-mission-sandbox-level)
                       (network . ,locus-mission-network)
                       (resources . ,locus-mission-resources)
                       (budget . ,locus-mission-budget)
                       (output_contract . ,locus-mission-output-contract)))))))
    (ignore rendu)
    (when (called-interactively-p 'interactive)
      (message "mission proposée : %s — `locus-mission-queue' pour la mettre en file"
               task-id))
    task-id))

;;;###autoload
(defun locus-mission-queue (task-id)
  "Mettre TASK-ID en file : c'est là qu'un worker peut le réclamer.

Séparé de la proposition, et c'est §15.2 : proposer est une écriture dans le
graphe, mettre en file est un engagement de ressources.  Les fondre ferait
qu'écrire une question la ferait exécuter."
  (interactive (list (read-string "Tâche à mettre en file : ")))
  (when (string-empty-p (string-trim task-id))
    (user-error "Aucune tâche nommée"))
  (locus-mission--post "commands/task/queue"
                       `((idempotency_key . ,(locus-mission--id "cmd"))
                   (project_id . ,(locus-mission-projet))
                         (task_id . ,task-id)))
  (when (called-interactively-p 'interactive)
    (message "%s est en file — un worker peut la réclamer" task-id))
  task-id)

;;;###autoload
(defun locus-mission-lancer (question conditions cognition)
  "Proposer QUESTION puis la mettre en file — les deux gestes, à la suite.

Le raccourci, pour le cas ordinaire où l'on écrit une mission parce qu'on veut
qu'elle tourne.  Il n'efface pas la séparation : les deux commandes existent
séparément, et une proposition qu'on ne met pas en file reste une écriture
valide dans le graphe."
  (interactive
   (list (read-string "La question : ")
         (split-string
          (read-string "À quoi on la reconnaîtra traitée (séparé par « ; ») : ")
          "[ \t]*;[ \t]*" t)
         (completing-read "Classe de cognition : " '("frontier" "economy") nil t
                          nil nil "frontier")))
  (let ((task-id (locus-mission-propose question conditions cognition)))
    (locus-mission-queue task-id)
    (message "mission %s proposée et en file" task-id)
    task-id))

(provide 'locus-mission)

;;; locus-mission.el ends here
