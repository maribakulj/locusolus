;;; locus-command.el --- Commandes mutantes : révision, conflit, idempotence  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; `SPEC.md' §11.
;;
;; # Le refus qui porte tout le module
;;
;; §11.3 : « **ne jamais** resoumettre automatiquement avec la nouvelle
;; révision ».  C'est la seule règle du fichier qui protège quelque chose
;; d'irrattrapable : resoumettre avec la révision courante applique la mutation
;; à un état que l'utilisateur n'a pas vu, et efface silencieusement le travail
;; de quelqu'un d'autre.  Le confort d'un retry automatique est réel ; ce qu'il
;; coûte ne se voit qu'après.
;;
;; La règle est donc rendue **observable** plutôt que promise : le transport est
;; un port, et le test compte ses appels.  Une resoumission serait un second
;; appel, et il n'y en a qu'un.
;;
;; # `expected_revision' n'est pas optionnel
;;
;; Une commande sans révision attendue est une commande qui écrase par
;; construction — elle réussit quel que soit l'état trouvé.  Le constructeur la
;; refuse, ce qui déplace la faute du moment de l'envoi, où elle est invisible,
;; au moment de l'écriture, où elle est évidente.

;;; Code:

(require 'cl-lib)
(require 'locus)

(define-error 'locus-command-invalid "Commande mal formée" 'locus-error)
(define-error 'locus-command-unconfirmed "Commande non confirmée" 'locus-error)

(defconst locus-command-severities '(safe controlled sensitive critical)
  "Les quatre niveaux de §11.2, du moindre au plus grave.")

(defun locus-command-requires-confirmation-p (severity)
  "Renvoyer non-nil quand SEVERITY exige une confirmation — §11.2."
  (memq severity '(sensitive critical)))

(defun locus-command-requires-reason-p (severity)
  "Renvoyer non-nil quand SEVERITY exige une raison écrite — §11.2.

Seul `critical' l'exige : suppression, fédération, changement admin.  Exiger
une raison partout ferait taper « ok » quatre fois par heure, et la raison
cesserait d'en être une."
  (eq severity 'critical))

(cl-defstruct (locus-command (:constructor locus-command--make) (:copier nil))
  "Une commande mutante, telle que §11.1 demande de la prévisualiser."
  type target expected-revision payload severity idempotency-key
  cost policy effects approvals reason)

(defun locus-command-create (type target expected-revision &rest options)
  "Construire une commande.

# Errors

`locus-command-invalid' quand TYPE, TARGET ou EXPECTED-REVISION manquent, quand
la sévérité n'est pas l'une des quatre, ou quand la clé d'idempotence est vide.
Une commande sans révision attendue écrase par construction ; une commande sans
clé d'idempotence ne peut pas être rejouée sans risque de doublon."
  (let ((severity (or (plist-get options :severity) 'controlled))
        (key (plist-get options :idempotency-key)))
    (unless (and type (stringp type) (not (string-empty-p type)))
      (signal 'locus-command-invalid (list "un type est requis")))
    (unless (and target (stringp target) (not (string-empty-p target)))
      (signal 'locus-command-invalid (list "une cible est requise")))
    (unless (and expected-revision (not (equal expected-revision "")))
      (signal 'locus-command-invalid
              (list "expected_revision est requise : sans elle la commande écrase")))
    (unless (memq severity locus-command-severities)
      (signal 'locus-command-invalid (list (format "sévérité inconnue : %s" severity))))
    (unless (and key (stringp key) (not (string-empty-p key)))
      (signal 'locus-command-invalid (list "une clé d'idempotence est requise")))
    (locus-command--make
     :type type :target target :expected-revision expected-revision
     :payload (plist-get options :payload)
     :severity severity
     :idempotency-key key
     :cost (plist-get options :cost)
     :policy (plist-get options :policy)
     :effects (plist-get options :effects)
     :approvals (plist-get options :approvals)
     :reason (plist-get options :reason))))

(defun locus-command-preview (command)
  "Les neuf lignes que §11.1 exige avant envoi.

Rend une alist plutôt qu'un texte : ce qui s'affiche se teste, et un
formateur qui oublierait un champ le ferait disparaître de la prévisualisation
sans que rien ne le dise."
  (list (cons :type (locus-command-type command))
        (cons :target (locus-command-target command))
        (cons :expected-revision (locus-command-expected-revision command))
        (cons :payload (locus-command-payload command))
        (cons :cost (locus-command-cost command))
        (cons :policy (locus-command-policy command))
        (cons :effects (locus-command-effects command))
        (cons :approvals (locus-command-approvals command))
        (cons :idempotency-key (locus-command-idempotency-key command))))

(defvar locus-command--known-results (make-hash-table :test #'equal)
  "Les résultats déjà obtenus, par clé d'idempotence — §11.4.")

(defun locus-command-forget-results ()
  "Oublier les résultats connus."
  (clrhash locus-command--known-results))

(defun locus-command-known-result (key)
  "Le résultat déjà obtenu pour KEY, ou nil — §11.4."
  (gethash key locus-command--known-results))

(defun locus-command-submit (command transport &rest options)
  "Soumettre COMMAND via TRANSPORT et rendre son issue.

TRANSPORT est un port : une fonction qui reçoit la commande et rend une alist
d'issue.  Le module n'ouvre aucune connexion — ce qui le rend testable, et ce
qui permet au test de **compter** les appels.

OPTIONS accepte `:confirmed'.

# Ce qui se passe sur conflit

L'issue est rendue telle quelle, avec l'état courant que le serveur a joint.
Rien n'est resoumis : §11.3 l'interdit, et c'est la seule règle de ce fichier
qui protège quelque chose d'irrattrapable.  Décider quoi faire — rafraîchir,
rebaser, réécrire — appartient à qui a vu le diff.

# Errors

`locus-command-unconfirmed' quand la sévérité exige une confirmation qui
manque, ou une raison qui manque."
  (let ((severity (locus-command-severity command)))
    (when (and (locus-command-requires-confirmation-p severity)
               (not (plist-get options :confirmed)))
      (signal 'locus-command-unconfirmed
              (list (format "une commande %s demande une confirmation explicite" severity))))
    (when (and (locus-command-requires-reason-p severity)
               (let ((reason (locus-command-reason command)))
                 (or (null reason) (string-empty-p (string-trim reason)))))
      (signal 'locus-command-unconfirmed
              (list "une commande critical demande une raison écrite"))))

  (let* ((key (locus-command-idempotency-key command))
         (known (locus-command-known-result key)))
    (if known
        ;; §11.4 : une réponse perdue ne se rejoue pas, elle se retrouve.
        (cons (cons :replayed t) known)
      (let ((outcome (funcall transport command)))
        (puthash key outcome locus-command--known-results)
        outcome))))

(defun locus-command-conflict-p (outcome)
  "Renvoyer non-nil quand OUTCOME est un conflit de révision."
  (eq (alist-get :status outcome) 'conflict))

(defun locus-command-rebased (command revision)
  "Une **nouvelle** commande, identique à COMMAND mais visant REVISION.

Existe pour que la reprise après conflit soit possible, et se fasse depuis un
appel explicite : §11.3 propose « refresh, rebase ou nouvelle commande » à
l'utilisateur, ce qui suppose que rebaser soit un geste et non un effet de
bord.  La clé d'idempotence change, sans quoi la nouvelle commande retrouverait
le résultat de l'ancienne au lieu de partir."
  (locus-command-create
   (locus-command-type command)
   (locus-command-target command)
   revision
   :payload (locus-command-payload command)
   :severity (locus-command-severity command)
   :idempotency-key (concat (locus-command-idempotency-key command) "+" (format "%s" revision))
   :cost (locus-command-cost command)
   :policy (locus-command-policy command)
   :effects (locus-command-effects command)
   :approvals (locus-command-approvals command)
   :reason (locus-command-reason command)))

(provide 'locus-command)

;;; locus-command.el ends here
