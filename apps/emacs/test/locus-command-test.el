;;; locus-command-test.el --- Test de sortie de W8.e  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Toute action mutante porte `expected_revision' ; un conflit est rendu, pas
;; écrasé.**
;;
;; La seconde moitié ne se vérifie pas en regardant ce que la fonction rend :
;; un module qui resoumettrait rendrait un succès, ce qui a l'air bien.  Le
;; test **compte les appels au transport**.  Une resoumission automatique
;; serait un second appel, et il n'y en a qu'un.

;;; Code:

(require 'ert)
(require 'cl-lib)
(require 'locus-command)

(defvar locus-command-test--calls nil
  "Les commandes reçues par le transport, dans l'ordre.")

(defun locus-command-test--transport (outcome)
  "Un transport qui enregistre son appel et rend OUTCOME."
  (lambda (command)
    (push command locus-command-test--calls)
    outcome))

(defun locus-command-test--reset ()
  "Repartir d'un état propre."
  (setq locus-command-test--calls nil)
  (locus-command-forget-results))

(defun locus-command-test--command (&rest options)
  "Une commande valide, ajustée par OPTIONS."
  (apply #'locus-command-create
         (or (plist-get options :type) "branch.rename")
         (or (plist-get options :target) "br-0001")
         (or (plist-get options :revision) 7)
         (append options (list :idempotency-key
                               (or (plist-get options :idempotency-key) "idem-1")))))

(defconst locus-command-test--conflict
  '((:status . conflict)
    (:current-revision . 9)
    (:diff . "le titre a changé"))
  "Ce qu'un serveur rend quand la révision attendue est obsolète.")

;; ------------------------------------------------------------------------
;; Un conflit est rendu, pas écrasé
;; ------------------------------------------------------------------------

(ert-deftest locus-command-un-conflit-ne-declenche-aucune-resoumission ()
  "Le refus qui porte le module — §11.3.

Resoumettre avec la révision courante appliquerait la mutation à un état que
l'utilisateur n'a pas vu, et effacerait silencieusement le travail de
quelqu'un d'autre.  Le confort d'un retry automatique est réel ; ce qu'il
coûte ne se voit qu'après."
  (locus-command-test--reset)
  (let* ((transport (locus-command-test--transport locus-command-test--conflict))
         (outcome (locus-command-submit (locus-command-test--command) transport)))

    (should (locus-command-conflict-p outcome))
    (ert-info ("un seul appel : une resoumission en serait un second")
      (should (equal (length locus-command-test--calls) 1)))))

(ert-deftest locus-command-le-conflit-porte-de-quoi-decider ()
  "§11.3 demande d'afficher l'état courant et de présenter le diff.  Un conflit
qui ne dirait que « conflit » obligerait à aller chercher ailleurs ce qu'il
faut pour choisir, et le choix serait fait sans."
  (locus-command-test--reset)
  (let ((outcome (locus-command-submit (locus-command-test--command)
                                       (locus-command-test--transport
                                        locus-command-test--conflict))))
    (should (equal (alist-get :current-revision outcome) 9))
    (should (stringp (alist-get :diff outcome)))))

(ert-deftest locus-command-rebaser-est-un-geste-explicite ()
  "§11.3 propose « refresh, rebase ou nouvelle commande » — à l'utilisateur.
Rebaser existe donc, et c'est un appel, pas un effet de bord."
  (locus-command-test--reset)
  (let* ((original (locus-command-test--command))
         (rebased (locus-command-rebased original 9)))
    (should (equal (locus-command-expected-revision rebased) 9))
    (should (equal (locus-command-type rebased) (locus-command-type original)))
    (ert-info ("la clé change, sans quoi la nouvelle commande retrouverait le résultat de l'ancienne")
      (should-not (equal (locus-command-idempotency-key rebased)
                         (locus-command-idempotency-key original))))))

;; ------------------------------------------------------------------------
;; `expected_revision' n'est pas optionnel
;; ------------------------------------------------------------------------

(ert-deftest locus-command-une-commande-sans-revision-attendue-est-refusee ()
  "Elle écrase par construction : elle réussit quel que soit l'état trouvé.  Le
refus est au constructeur, ce qui déplace la faute du moment de l'envoi, où
elle est invisible, à celui de l'écriture, où elle est évidente."
  (should-error (locus-command-create "branch.rename" "br-1" nil :idempotency-key "k")
                :type 'locus-command-invalid)
  (should-error (locus-command-create "branch.rename" "br-1" "" :idempotency-key "k")
                :type 'locus-command-invalid))

(ert-deftest locus-command-une-commande-sans-cle-d-idempotence-est-refusee ()
  "Sans clé, une réponse perdue ne peut être retrouvée que par une nouvelle
soumission — c'est-à-dire par un doublon possible."
  (should-error (locus-command-create "branch.rename" "br-1" 7)
                :type 'locus-command-invalid))

(ert-deftest locus-command-le-type-et-la-cible-sont-exiges ()
  (should-error (locus-command-create "" "br-1" 7 :idempotency-key "k")
                :type 'locus-command-invalid)
  (should-error (locus-command-create "branch.rename" "" 7 :idempotency-key "k")
                :type 'locus-command-invalid))

;; ------------------------------------------------------------------------
;; La confirmation graduée
;; ------------------------------------------------------------------------

(ert-deftest locus-command-les-quatre-niveaux-de_11_2-existent ()
  (should (equal locus-command-severities '(safe controlled sensitive critical))))

(ert-deftest locus-command-une-commande-sensible-ne-part-pas-sans-confirmation ()
  "§11.2 : `sensitive' — coût, publication, merge, secret, données."
  (locus-command-test--reset)
  (let ((transport (locus-command-test--transport '((:status . ok)))))
    (should-error (locus-command-submit (locus-command-test--command :severity 'sensitive)
                                        transport)
                  :type 'locus-command-unconfirmed)
    (ert-info ("le refus précède l'envoi : le transport n'a rien vu")
      (should (null locus-command-test--calls)))
    (should (locus-command-submit (locus-command-test--command :severity 'sensitive)
                                  transport :confirmed t))))

(ert-deftest locus-command-une-commande-critique-demande-une-raison ()
  "§11.2 : `critical' — suppression, fédération, changement admin ; formulaire
explicite **et raison**.  Une confirmation seule ne suffit pas."
  (locus-command-test--reset)
  (let ((transport (locus-command-test--transport '((:status . ok)))))
    (should-error (locus-command-submit (locus-command-test--command :severity 'critical)
                                        transport :confirmed t)
                  :type 'locus-command-unconfirmed)
    (should (null locus-command-test--calls))
    (should (locus-command-submit
             (locus-command-test--command :severity 'critical :reason "dépôt fermé")
             transport :confirmed t))))

(ert-deftest locus-command-une-raison-blanche-n-est-pas-une-raison ()
  "Un formulaire qu'on remplit d'espaces est un formulaire qu'on contourne."
  (locus-command-test--reset)
  (should-error (locus-command-submit
                 (locus-command-test--command :severity 'critical :reason "   ")
                 (locus-command-test--transport '((:status . ok)))
                 :confirmed t)
                :type 'locus-command-unconfirmed))

(ert-deftest locus-command-les-niveaux-bas-partent-sans-ceremonie ()
  "Exiger une raison partout ferait taper « ok » quatre fois par heure, et la
raison cesserait d'en être une.  `safe' et `controlled' passent."
  (locus-command-test--reset)
  (let ((transport (locus-command-test--transport '((:status . ok)))))
    (should (locus-command-submit (locus-command-test--command :severity 'safe) transport))
    (locus-command-forget-results)
    (should (locus-command-submit (locus-command-test--command :severity 'controlled)
                                  transport))))

;; ------------------------------------------------------------------------
;; La prévisualisation
;; ------------------------------------------------------------------------

(ert-deftest locus-command-la-previsualisation-porte-les-neuf-champs-de_11_1 ()
  "Rendue en alist plutôt qu'en texte : ce qui s'affiche se teste, et un
formateur qui oublierait un champ le ferait disparaître sans que rien ne le
dise."
  (let* ((command (locus-command-test--command
                   :payload '((:name . "nouveau"))
                   :cost "3 appels" :policy "pol-1"
                   :effects '("renomme la branche") :approvals '("humain")))
         (preview (locus-command-preview command)))
    (dolist (field '(:type :target :expected-revision :payload :cost
                     :policy :effects :approvals :idempotency-key))
      (ert-info ((format "champ %s" field))
        (should (assq field preview))))
    (should (equal (alist-get :expected-revision preview) 7))))

;; ------------------------------------------------------------------------
;; L'idempotence
;; ------------------------------------------------------------------------

(ert-deftest locus-command-une-reponse-connue-se-retrouve-elle-ne-se-rejoue-pas ()
  "§11.4 : le client affiche le résultat connu si une réponse a été perdue.

Le transport n'est pas rappelé : rejouer une mutation pour retrouver sa
réponse est exactement le doublon que la clé d'idempotence existe pour
éviter."
  (locus-command-test--reset)
  (let ((transport (locus-command-test--transport '((:status . ok) (:revision . 8)))))
    (locus-command-submit (locus-command-test--command) transport)
    (should (equal (length locus-command-test--calls) 1))

    (let ((again (locus-command-submit (locus-command-test--command) transport)))
      (should (equal (length locus-command-test--calls) 1))
      (should (alist-get :replayed again))
      (should (equal (alist-get :revision again) 8)))))

(ert-deftest locus-command-deux-cles-distinctes-sont-deux-commandes ()
  "Sans cela, l'idempotence bloquerait la seconde commande légitime."
  (locus-command-test--reset)
  (let ((transport (locus-command-test--transport '((:status . ok)))))
    (locus-command-submit (locus-command-test--command :idempotency-key "a") transport)
    (locus-command-submit (locus-command-test--command :idempotency-key "b") transport)
    (should (equal (length locus-command-test--calls) 2))))

(ert-deftest locus-command-le-transport-est-un-port ()
  "Aucune connexion n'est ouverte ici : c'est ce qui rend le module testable, et
c'est ce qui permet au test de compter les appels."
  (locus-command-test--reset)
  (let ((vu nil))
    (locus-command-submit (locus-command-test--command)
                          (lambda (command) (setq vu command) '((:status . ok))))
    (should (locus-command-p vu))
    (should (equal (locus-command-expected-revision vu) 7))))

(provide 'locus-command-test)

;;; locus-command-test.el ends here
