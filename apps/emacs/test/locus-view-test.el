;;; locus-view-test.el --- Test de sortie de W8.h  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **La 3D reste une projection ; aucune vue n'écrit dans le graphe.**
;;
;; La seconde moitié ne se vérifie pas en cherchant une fonction d'écriture :
;; il n'y en a pas, et c'est précisément le problème — une projection qui
;; partagerait ses structures avec la source ferait du premier `setcdr' d'une
;; vue une écriture dans le graphe, sans qu'aucune ligne ne s'appelle
;; « écrire ».
;;
;; Le test **abîme la projection** et regarde la source.

;;; Code:

(require 'ert)
(require 'cl-lib)
(require 'locus-view)

(defun locus-view-test--graph (&optional count)
  "Un graphe de COUNT nœuds, chaînés."
  (let* ((n (or count 3))
         (nodes (cl-loop for i from 1 to n
                         collect (list (cons :id (format "n%d" i))
                                       (cons :label (format "nœud %d" i))
                                       (cons :state "validated"))))
         (edges (cl-loop for i from 1 below n
                         collect (list (cons :from (format "n%d" i))
                                       (cons :to (format "n%d" (1+ i)))
                                       (cons :kind "supports")))))
    (list (cons :nodes nodes) (cons :edges edges))))

;; ------------------------------------------------------------------------
;; Aucune vue n'écrit dans le graphe
;; ------------------------------------------------------------------------

(ert-deftest locus-view-abimer-la-projection-ne-touche-pas-le-graphe ()
  "Le test qui porte le sprint.

`docs/10' : « le service produit une projection, jamais une copie mutable du
graphe ». Une copie de surface laisserait les alists de nœuds partagées, et le
premier `setcdr' d'une vue écrirait dans le graphe — sans qu'aucune ligne de
code ne s'appelle « écrire », donc sans rien à voir dans un diff."
  (let* ((graph (locus-view-test--graph 3))
         (avant (copy-tree graph))
         (view (locus-view-project graph)))

    ;; On abîme la projection de toutes les façons possibles.
    (let ((node (car (locus-view-nodes view))))
      (setcdr (assq :label node) "SACCAGÉ")
      (setcdr (assq :state node) "promoted"))
    (setcar (locus-view-nodes view) '((:id . "injecté")))
    (let ((edge (car (locus-view-edges view))))
      (setcdr (assq :kind edge) "refutes"))

    (ert-info ("le graphe source est intact, à l'identique")
      (should (equal graph avant)))))

(ert-deftest locus-view-abimer-le-graphe-ne-touche-pas-la-projection ()
  "Le détachement vaut dans les deux sens : une vue affichée doit continuer de
montrer ce qu'elle montrait, sans quoi elle changerait sous les yeux du lecteur
au premier événement reçu."
  (let* ((graph (locus-view-test--graph 3))
         (view (locus-view-project graph))
         (avant (copy-tree (locus-view-nodes view))))
    (setcdr (assq :label (car (alist-get :nodes graph))) "AUTRE")
    (should (equal (locus-view-nodes view) avant))))

(ert-deftest locus-view-une-action-est-une-description-pas-une-fermeture ()
  "Une fermeture pourrait faire n'importe quoi, et le contrôle de §11 —
`expected_revision', confirmation graduée, conflit rendu — serait contourné par
le chemin le plus court."
  (let ((graph (locus-view-test--graph 2)))
    (should (locus-view-project graph
                                :actions '(((:command . "branch.rename")
                                            (:target . "br-1")))))
    (should-error (locus-view-project graph :actions (list (lambda () (ignore))))
                  :type 'locus-view-refused)
    (should-error (locus-view-project graph :actions (list #'delete-file))
                  :type 'locus-view-refused)))

;; ------------------------------------------------------------------------
;; La troncature se dit
;; ------------------------------------------------------------------------

(ert-deftest locus-view-une-vue-tronquee-le-dit ()
  "§13.2 : « limite le nombre de nœuds ». Une vue tronquée **sans le dire** se
lit comme un graphe complet, et c'est la conclusion qu'on en tire qui est
fausse, pas l'affichage."
  (let ((locus-view-max-nodes 5))
    (let ((view (locus-view-project (locus-view-test--graph 12))))
      (should (equal (length (locus-view-nodes view)) 5))
      (should (locus-view-truncated-p view))
      (should (equal (locus-view-truncated view) 7))
      (ert-info ("le total reste connu : sans lui, « 7 de plus » ne se situe pas")
        (should (equal (locus-view-total view) 12))))))

(ert-deftest locus-view-une-vue-complete-ne-se-declare-pas-tronquee ()
  "Sans ce cas, « dire la troncature » pourrait vouloir dire « dire toujours »."
  (let ((locus-view-max-nodes 50))
    (let ((view (locus-view-project (locus-view-test--graph 3))))
      (should-not (locus-view-truncated-p view))
      (should (equal (locus-view-truncated view) 0))
      (should (equal (locus-view-total view) 3)))))

(ert-deftest locus-view-une-arete-vers-un-noeud-ecarte-ne-survit-pas ()
  "Garder une arête vers un nœud tronqué dessinerait un lien vers rien, ce
qu'une vue rendrait indistinguable d'une relation cassée."
  (let ((locus-view-max-nodes 3))
    (let* ((view (locus-view-project (locus-view-test--graph 6)))
           (ids (mapcar (lambda (node) (alist-get :id node)) (locus-view-nodes view))))
      (should (equal (length (locus-view-nodes view)) 3))
      (dolist (edge (locus-view-edges view))
        (ert-info ((format "arête %s → %s" (alist-get :from edge) (alist-get :to edge)))
          (should (member (alist-get :from edge) ids))
          (should (member (alist-get :to edge) ids)))))))

;; ------------------------------------------------------------------------
;; Le handoff est une charge, pas un canal
;; ------------------------------------------------------------------------

(ert-deftest locus-view-le-handoff-ne-porte-aucune-fonction ()
  "Un viewer qui ne peut recevoir qu'un document ne peut pas recevoir de
pouvoir.  La charge est donc sérialisable — et un rappel qui reviendrait écrire
serait la faute de §13.2 à travers une fenêtre."
  (let* ((view (locus-view-project (locus-view-test--graph 3)
                                   :actions '(((:command . "branch.rename")))))
         (payload (locus-view-handoff view)))
    (should (locus-view-test--free-of-functions-p payload))
    (ert-info ("et pas d'actions non plus : les proposer inviterait à les déclencher")
      (should-not (assq :actions payload)))
    (should (alist-get :read-only payload))))

(defun locus-view-test--free-of-functions-p (value)
  "Renvoyer non-nil quand VALUE ne contient aucune fonction, même imbriquée."
  (cond
   ((functionp value) nil)
   ((consp value) (and (locus-view-test--free-of-functions-p (car value))
                       (locus-view-test--free-of-functions-p (cdr value))))
   (t t)))

(ert-deftest locus-view-le-handoff-porte-ce-qui-s-affiche-troncature-comprise ()
  "Le viewer externe doit savoir qu'il ne montre pas tout, sinon la troncature
serait dite dans Emacs et tue dans la fenêtre — c'est-à-dire tue là où on
regarde."
  (let ((locus-view-max-nodes 2))
    (let ((payload (locus-view-handoff (locus-view-project (locus-view-test--graph 9)))))
      (should (equal (alist-get :truncated payload) 7))
      (should (equal (alist-get :total payload) 9))
      (should (equal (length (alist-get :nodes payload)) 2)))))

(ert-deftest locus-view-abimer-la-charge-ne-touche-pas-le-graphe ()
  "La charge vient de la projection, qui est déjà détachée : ce test le
constate de bout en bout, du graphe au viewer."
  (let* ((graph (locus-view-test--graph 3))
         (avant (copy-tree graph))
         (payload (locus-view-handoff (locus-view-project graph))))
    (setcdr (assq :label (car (alist-get :nodes payload))) "SACCAGÉ")
    (should (equal graph avant))))

(provide 'locus-view-test)

;;; locus-view-test.el ends here
