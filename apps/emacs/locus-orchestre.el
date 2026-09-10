;;; locus-orchestre.el --- Enchaîner des missions, et les suivre  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Un plan de plusieurs missions s'exécute sans qu'on soumette chaque étape.**
;;
;; `locus-mission.el' sait proposer une mission et la mettre en file.  Il ne sait
;; pas ce qui vient après : une mission finit, et personne ne lance la suivante.
;; Tant que cela dure, un travail en plusieurs temps se pilote à la main, une
;; commande par étape — ce qui n'est pas de l'orchestration, c'est une liste de
;; courses.
;;
;; # Ce que ce module fait, et ce qu'il ne fait pas
;;
;; Il tient un **plan** — une suite d'étapes — et le fait avancer : il soumet la
;; première, guette la fin dans le journal, soumet la suivante.  C'est tout, et
;; c'est délibérément peu.
;;
;; Il ne décide pas *quel agent* traite quoi : c'est le placement du daemon qui
;; le fait, sur ce que les workers annoncent.  Une étape déclare ce qu'elle
;; **exige** — `required_capabilities` de §15.4 — et un hôte qui ne l'a pas
;; refuse la mission, avec un motif.  Choisir ici reviendrait à décider à la
;; place du placement, avec moins d'informations que lui.
;;
;; # L'avancement se lit dans le journal, jamais dans une variable
;;
;; Un orchestrateur qui garderait son propre état d'avancement en aurait deux :
;; le sien et celui du daemon.  Ils divergeraient au premier redémarrage
;; d'Emacs, et c'est celui du daemon qui a raison.  Ce module retient donc **le
;; plan** et **l'identifiant de l'étape en cours**, et rien d'autre : l'état de
;; cette étape se relit à chaque tour.
;;
;; # Ce qui passe d'une étape à l'autre
;;
;; La **question** de l'étape suivante, augmentée de ce que la précédente a
;; rendu.  Rien de plus : le contexte institutionnel voyage par la vue de
;; contexte, qui est ce que §16.2 prévoit pour ça, et recopier un résultat
;; entier dans une question ferait grossir le prompt à chaque étape — ce que le
;; budget paie en jetons d'entrée, qui sont déjà le poste principal.

;;; Code:

(require 'cl-lib)
(require 'locus)
(require 'locus-cache)
(require 'locus-session)
(require 'locus-mission)

(defgroup locus-orchestre nil
  "Enchaîner des missions."
  :group 'locus
  :prefix "locus-orchestre-")

(defcustom locus-orchestre-interval 5
  "Secondes entre deux relectures de l'avancement.

Cinq, et non trois comme le cockpit : celui-ci regarde, celui-là **agit**.
Un tour qui soumettrait une étape est plus coûteux qu'un tour qui redessine,
et l'avancement d'une mission se compte en minutes."
  :type 'integer
  :group 'locus-orchestre)

(cl-defstruct (locus-etape (:constructor locus-etape-creer)
                           (:copier nil))
  "Une étape d'un plan.

QUESTION et CONDITIONS sont ce que §15.4 appelle l'objectif.  CAPACITES est
`required_capabilities' : ce que l'hôte doit savoir faire — « vision » pour
lire une planche, et le placement refuse un worker qui ne l'annonce pas."
  question
  conditions
  (cognition "economy")
  (capacites nil))

(defvar locus-orchestre--plan nil
  "Les étapes qui restent à soumettre, la prochaine en tête.")

(defvar locus-orchestre--courante nil
  "L'identifiant de la tâche en cours, ou nil.")

(defvar locus-orchestre--faites nil
  "Les étapes achevées, en (IDENTIFIANT . ÉTAT), la plus récente en tête.")

(defvar locus-orchestre--timer nil
  "Le minuteur d'avancement, ou nil.")

(defvar locus-orchestre--nom nil
  "Le nom du plan en cours, pour les messages.")

(defconst locus-orchestre-buffer "*Locus Orchestre*"
  "Où le déroulé s'écrit.")

(defun locus-orchestre--journal (format &rest args)
  "Écrire une ligne datée dans le tampon d'orchestration."
  (with-current-buffer (get-buffer-create locus-orchestre-buffer)
    (let ((inhibit-read-only t))
      (goto-char (point-max))
      (insert (propertize (format-time-string "%H:%M:%S  ") 'face 'shadow)
              (apply #'format format args) "\n"))))

(defun locus-orchestre--etat (task-id)
  "L'état de TASK-ID selon le journal déjà en cache, ou nil s'il est absent.

Lit le cache et rien d'autre — même règle que le cockpit.  Une requête d'ici
ferait deux lectures du même journal par tour, dont une pour rien."
  (let ((mission (assoc task-id (locus-session--missions
                                 (locus-session--items "timeline")))))
    (and mission (nth 1 mission))))

(defun locus-orchestre--soumettre (etape &optional precedent)
  "Soumettre ETAPE, en lui joignant ce que PRECEDENT a rendu.

PRECEDENT est le texte de sortie de l'étape d'avant, ou nil pour la première."
  (let ((question (if precedent
                      (concat (locus-etape-question etape)
                              "\n\nCe que l'étape précédente a établi :\n"
                              precedent)
                    (locus-etape-question etape))))
    (locus-mission-lancer question
                          (locus-etape-conditions etape)
                          (locus-etape-cognition etape))))

(defun locus-orchestre--avancer ()
  "Un tour : relire, et soumettre la suite si l'étape en cours est finie.

# Pourquoi la relecture est ici et pas dans le cockpit

Les deux tournent peut-être en même temps, et faire dépendre l'avancement du
mode direct rendrait l'orchestration muette dès qu'on ferme le cockpit — ce
qu'on fait précisément quand on le laisse travailler."
  (locus-session-refresh)
  (let ((etat (and locus-orchestre--courante
                   (locus-orchestre--etat locus-orchestre--courante))))
    (cond
     ;; Rien en cours : soumettre la prochaine, ou finir.
     ((null locus-orchestre--courante)
      (if (null locus-orchestre--plan)
          (locus-orchestre-arreter "plan terminé")
        (let ((etape (pop locus-orchestre--plan)))
          (setq locus-orchestre--courante
                (locus-orchestre--soumettre etape (locus-orchestre--dernier-resultat)))
          (locus-orchestre--journal "→ étape soumise : %s — %s"
                                    locus-orchestre--courante
                                    (locus-etape-question etape)))))
     ;; L'étape a abouti : on la range et le tour suivant soumettra la suite.
     ((equal etat "terminée")
      (push (cons locus-orchestre--courante etat) locus-orchestre--faites)
      (locus-orchestre--journal "✓ %s terminée" locus-orchestre--courante)
      (setq locus-orchestre--courante nil))
     ;; Un échec **arrête le plan**.  Enchaîner sur une étape dont la
     ;; précédente n'a rien établi ferait travailler la suite sur du vide, et
     ;; le budget paierait chaque étape suivante pour rien.
     ((member etat '("ÉCHOUÉE" "refusée"))
      (push (cons locus-orchestre--courante etat) locus-orchestre--faites)
      (locus-orchestre--journal "✗ %s : %s — plan arrêté"
                                locus-orchestre--courante etat)
      (setq locus-orchestre--courante nil)
      (locus-orchestre-arreter (format "étape %s" etat)))
     ;; Elle tourne, ou le journal ne la connaît pas encore.
     (t nil))))

(defcustom locus-orchestre-relais-max 4000
  "Longueur maximale du texte passé d'une étape à la suivante, en caractères.

Un résultat entier peut faire plusieurs pages, et il est **payé en jetons
d'entrée** par chaque étape suivante — le poste principal du budget, mesuré à
près de 20 000 jetons par appel avant même la question.  La borne est
volontairement large : couper trop court ferait perdre la substance, ce qui
coûte une étape entière plutôt que quelques jetons."
  :type 'integer
  :group 'locus-orchestre)

(defun locus-orchestre--texte-de-sortie (sortie)
  "Le texte lisible d'une SORTIE d'attempt, ou nil.

La sortie est un objet libre : le protocole n'impose pas sa forme, et deux
workers peuvent la remplir différemment.  On cherche donc les champs qui
portent d'ordinaire de la prose, dans l'ordre, et on retombe sur la
sérialisation entière plutôt que de rendre nil — un objet lisible mal formé
vaut mieux qu'un relais silencieusement vide."
  (when sortie
    (or (alist-get 'summary sortie)
        (alist-get 'text sortie)
        (alist-get 'output sortie)
        (let ((brut (json-serialize sortie)))
          (and (stringp brut) brut)))))

(defun locus-orchestre--resultat (task-id)
  "Ce que TASK-ID a rendu, relu du daemon, ou nil.

`GET /tasks/{id}/result` répond 404 tant qu'aucun attempt n'a abouti — ce qui
n'est pas une erreur ici : on demande justement pour savoir."
  (condition-case nil
      (let ((rendu (locus-session-get (format "tasks/%s/result" task-id))))
        (locus-orchestre--texte-de-sortie (alist-get 'output rendu)))
    (locus-session-unreachable nil)))

(defun locus-orchestre--dernier-resultat ()
  "Ce que la dernière étape achevée a rendu, tronqué à `locus-orchestre-relais-max'.

Le contenu, pas l'identifiant.  La première rédaction passait « la tâche X,
achevée » — ce qui n'est pas un relais : l'étape suivante repartait à vide et
répondait à côté.  Mesuré : une étape à qui l'on demandait où chercher des
manifestes IIIF pour trois manuscrits nommés à l'étape d'avant a répondu par
une liste générale de portails, faute de savoir de quels manuscrits on parlait."
  (when locus-orchestre--faites
    (let ((texte (locus-orchestre--resultat (car (car locus-orchestre--faites)))))
      (cond
       ((null texte) nil)
       ((<= (length texte) locus-orchestre-relais-max) texte)
       (t (concat (substring texte 0 locus-orchestre-relais-max)
                  "\n[…] (résultat tronqué)"))))))

;;;###autoload
(defun locus-orchestre-lancer (nom etapes)
  "Lancer le plan NOM, fait des ETAPES, et le suivre jusqu'au bout.

Un seul plan à la fois : deux plans concurrents partageraient le budget et la
file du daemon sans que ni l'un ni l'autre ne le sache, et c'est le genre de
concurrence qu'on ne veut pas découvrir sur une facture."
  (when locus-orchestre--timer
    (user-error "un plan tourne déjà : %s" (or locus-orchestre--nom "sans nom")))
  (unless etapes
    (user-error "un plan sans étape n'a rien à faire"))
  (setq locus-orchestre--nom nom
        locus-orchestre--plan (copy-sequence etapes)
        locus-orchestre--courante nil
        locus-orchestre--faites nil)
  (with-current-buffer (get-buffer-create locus-orchestre-buffer)
    (let ((inhibit-read-only t))
      (erase-buffer)
      (special-mode)))
  (locus-orchestre--journal "plan « %s » — %d étape(s)" nom (length etapes))
  ;; Un premier tour tout de suite : attendre l'intervalle pour soumettre la
  ;; première étape ferait passer cinq secondes où rien ne se voit, et un
  ;; lancement qui ne fait rien tout de suite se lit comme un lancement raté.
  (locus-orchestre--avancer)
  (setq locus-orchestre--timer
        (run-with-timer locus-orchestre-interval locus-orchestre-interval
                        #'locus-orchestre--avancer))
  (display-buffer locus-orchestre-buffer)
  nom)

;;;###autoload
(defun locus-orchestre-arreter (&optional motif)
  "Arrêter le plan en cours, pour MOTIF.

N'annule **pas** la mission en vol : elle a été acceptée par le daemon, elle a
un bail, et la retirer d'ici laisserait un worker travailler pour un plan qui
n'existe plus.  Arrêter l'orchestration et arrêter une mission sont deux gestes,
et le second passe par le daemon."
  (interactive)
  (when (timerp locus-orchestre--timer)
    (cancel-timer locus-orchestre--timer))
  (setq locus-orchestre--timer nil)
  (locus-orchestre--journal "plan « %s » arrêté%s"
                            (or locus-orchestre--nom "sans nom")
                            (if motif (format " — %s" motif) ""))
  (when (called-interactively-p 'interactive)
    (message "locus : plan arrêté")))

;;;###autoload
(defun locus-orchestre-etat ()
  "Où en est le plan : rendu **et** affiché."
  (interactive)
  (let ((phrase
         (if (null locus-orchestre--timer)
             "aucun plan en cours"
           (format "plan « %s » : %d faite(s), %s, %d restante(s)"
                   locus-orchestre--nom
                   (length locus-orchestre--faites)
                   (if locus-orchestre--courante
                       (format "%s en cours" locus-orchestre--courante)
                     "aucune en cours")
                   (length locus-orchestre--plan)))))
    (when (called-interactively-p 'interactive)
      (message "locus : %s" phrase))
    phrase))

(provide 'locus-orchestre)

;;; locus-orchestre.el ends here
