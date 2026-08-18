;;; locus-view.el --- Les vues du graphe : des projections, jamais des copies mutables  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; `SPEC.md' §13.2, et la contrainte que `docs/10' inscrit en tête de W9 :
;; « le service produit une **projection**, jamais une copie mutable du graphe.
;; Si une vue devient éditable en place, l'invariant *aucun frontend n'écrit
;; directement dans le graphe* est perdu. »
;;
;; # Ce que « projection » veut dire ici, concrètement
;;
;; Deux choses, et il faut les deux.
;;
;; La projection est **détachée** : modifier ce qu'on a reçu ne modifie pas le
;; graphe.  Une projection qui partagerait ses structures avec la source ferait
;; du premier `setcdr' d'une vue une écriture dans le graphe — et il n'y aurait
;; rien à lire dans le diff qui le dise, puisque personne n'aurait écrit de
;; fonction pour cela.
;;
;; Et la projection ne porte **aucun moyen d'écrire** : ses actions sont des
;; descriptions de commandes, pas des fermetures qui muteraient.  Une action qui
;; serait une fermeture pourrait faire n'importe quoi, et le contrôle de §11 —
;; `expected_revision', confirmation graduée, conflit rendu — serait contourné
;; par le chemin le plus court.
;;
;; # La 3D ne change rien à cela
;;
;; Le handoff vers un viewer externe est une **charge utile**, pas un canal :
;; ce qui part est ce qui s'affiche, et rien n'est prévu pour revenir.  Un
;; rappel qui reviendrait écrire serait la même faute, à travers une fenêtre.

;;; Code:

(require 'cl-lib)
(require 'locus)

(define-error 'locus-view-refused "Projection refusée" 'locus-error)

(defcustom locus-view-max-nodes 200
  "Nombre maximal de nœuds projetés — `SPEC.md' §13.2, « limite le nombre de nœuds ».

Au-delà, une vue cesse d'être lisible avant de cesser d'être calculable : la
limite protège la lecture, pas la machine."
  :type 'integer
  :group 'locus)

(cl-defstruct (locus-view (:constructor locus-view--make) (:copier nil))
  "Une projection du graphe.

Aucun accesseur ne rend une structure partagée avec la source, et aucun champ
ne porte de fonction : voir le commentaire d'en-tête."
  nodes edges truncated total selection actions)

(defun locus-view--detach (value)
  "Une copie profonde de VALUE.

Le détachement est ce qui fait la projection.  Une copie de surface laisserait
les alists de nœuds partagées avec la source, et modifier un nœud de la vue
écrirait dans le graphe sans qu'aucune ligne de code ne s'appelle « écrire »."
  (cond
   ((consp value) (cons (locus-view--detach (car value))
                        (locus-view--detach (cdr value))))
   ((vectorp value) (vconcat (mapcar #'locus-view--detach value)))
   ((stringp value) (copy-sequence value))
   (t value)))

(defun locus-view-project (graph &rest options)
  "Projeter GRAPH.

GRAPH est une alist portant `:nodes' et `:edges'.  OPTIONS accepte
`:selection' et `:actions'.

# Errors

`locus-view-refused' quand une action n'est pas une **description** de
commande.  Une action qui serait une fermeture pourrait faire n'importe quoi,
et le contrôle de §11 serait contourné par le chemin le plus court."
  (let* ((nodes (alist-get :nodes graph))
         (edges (alist-get :edges graph))
         (actions (plist-get options :actions))
         (total (length nodes))
         (kept (if (> total locus-view-max-nodes)
                   (take locus-view-max-nodes nodes)
                 nodes)))
    (dolist (action actions)
      (when (functionp action)
        (signal 'locus-view-refused
                (list "une action de vue est une description de commande, pas une fonction"))))
    (locus-view--make
     :nodes (locus-view--detach kept)
     :edges (locus-view--detach (locus-view--edges-among kept edges))
     ;; La troncature est **dite**, pas silencieuse : une vue tronquée sans le
     ;; dire se lit comme un graphe complet, et c'est la conclusion qu'on en
     ;; tire qui est fausse, pas l'affichage.
     :truncated (max 0 (- total locus-view-max-nodes))
     :total total
     :selection (locus-view--detach (plist-get options :selection))
     :actions (locus-view--detach actions))))

(defun locus-view--edges-among (nodes edges)
  "Les EDGES dont les deux extrémités sont dans NODES.

Garder une arête vers un nœud écarté dessinerait un lien vers rien, ce qu'une
vue tronquée rendrait indistinguable d'une relation cassée."
  (let ((ids (mapcar (lambda (node) (alist-get :id node)) nodes)))
    (seq-filter (lambda (edge)
                  (and (member (alist-get :from edge) ids)
                       (member (alist-get :to edge) ids)))
                edges)))

(defun locus-view-truncated-p (view)
  "Renvoyer non-nil quand VIEW ne montre pas tout."
  (> (locus-view-truncated view) 0))

(defun locus-view-handoff (view)
  "La charge utile à remettre à un viewer externe — 3D, WebView, navigateur.

Une **charge**, pas un canal : ce qui part est ce qui s'affiche, et rien n'est
prévu pour revenir.  Un rappel qui reviendrait écrire serait la faute de §13.2
à travers une fenêtre.

La charge ne porte aucune fonction, ce qui la rend sérialisable — et un
viewer qui ne peut recevoir qu'un document ne peut pas recevoir de pouvoir."
  (list (cons :nodes (locus-view-nodes view))
        (cons :edges (locus-view-edges view))
        (cons :truncated (locus-view-truncated view))
        (cons :total (locus-view-total view))
        (cons :selection (locus-view-selection view))
        ;; Pas d'`:actions' : une charge utile qui porterait des actions
        ;; inviterait le viewer à les déclencher, alors que §11 veut qu'elles
        ;; passent par la confirmation graduée et `expected_revision'.
        (cons :read-only t)))

(provide 'locus-view)

;;; locus-view.el ends here
