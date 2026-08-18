;;; locus-artifact.el --- Artefacts : hash vérifié, rien d'exécuté  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; `SPEC.md' §21.1 et §21.2, et les états de `packages/artifacts' (W6.a).
;;
;; # Ce qui est promu se voit, et ce qui ne l'est pas se voit aussi
;;
;; C'est la propriété de sortie du sprint, et elle est visuelle parce que la
;; faute l'est : un artefact `staged' affiché comme un artefact `promoted' fait
;; citer un résultat qui n'a pas été validé.  L'invariant 4 du dépôt — tout
;; résultat majeur est artifact-first et provenance-first — ne tient pas si
;; l'écran aplatit la différence.
;;
;; Un badge par état, donc, et **aucun défaut** : un état inconnu ne se rend pas
;; comme le plus rassurant, il se rend comme inconnu.
;;
;; # Rien n'est exécuté, et rien n'est cru sur parole
;;
;; §21.2 : « fichier non exécuté automatiquement », « hash vérifié », « quarantaine
;; si type douteux ».  Les trois protègent la même chose : un artefact vient
;; d'une exécution non fiable, et le client est la machine de l'utilisateur.
;;
;; La vérification du hash **précède** l'ouverture, et non l'inverse : ouvrir
;; puis vérifier laisse un fichier écrit sur le disque, ce que la vérification
;; ne défait pas.

;;; Code:

(require 'cl-lib)
(require 'locus)

(define-error 'locus-artifact-refused "Artefact refusé" 'locus-error)

(defconst locus-artifact-states
  '(declared uploaded quarantined verified promoted rejected)
  "Les six états de `packages/artifacts' — la même énumération, sous le même nom.

Le client n'invente pas de vocabulaire : un état affiché qui n'existe pas côté
serveur serait une information que rien ne peut confirmer.")

(defconst locus-artifact-servable-states '(verified promoted)
  "Les états dont le contenu peut être servi.")

(defun locus-artifact-badge (state)
  "Le badge affiché pour STATE.

Aucun défaut rassurant : un état inconnu rend « ? » et non le badge le plus
neutre.  Rendre l'inconnu comme du connu est la façon la plus discrète de
faire citer un résultat non validé."
  (pcase state
    ('promoted "✓ promu")
    ('verified "· vérifié")
    ('uploaded "⋯ déposé")
    ('declared "⋯ déclaré")
    ('quarantined "⚠ quarantaine")
    ('rejected "✗ rejeté")
    (_ "? inconnu")))

(defun locus-artifact-promoted-p (state)
  "Renvoyer non-nil quand STATE est `promoted'."
  (eq state 'promoted))

(defun locus-artifact-servable-p (state)
  "Renvoyer non-nil quand le contenu de STATE peut être servi."
  (and (memq state locus-artifact-servable-states) t))

(defconst locus-artifact-suspect-extensions
  '("exe" "dll" "so" "dylib" "sh" "bash" "zsh" "bat" "cmd" "ps1" "scr" "com"
    "el" "elc" "py" "rb" "pl" "jar" "app")
  "Les extensions qui mettent un artefact en quarantaine — §21.2.

La liste est volontairement large : le coût d'une quarantaine indue est une
commande de plus, celui d'un faux négatif est une exécution sur la machine de
l'utilisateur.  Elle n'est pas non plus une garantie — c'est pour cela que
rien n'est exécuté, quelle que soit l'extension.")

(defun locus-artifact-suspect-p (filename)
  "Renvoyer non-nil quand FILENAME est d'un type douteux — §21.2."
  (let ((extension (file-name-extension (or filename ""))))
    (and extension
         (member (downcase extension) locus-artifact-suspect-extensions)
         t)))

(defun locus-artifact-verify (declared-hash content hasher)
  "Confronter CONTENT au DECLARED-HASH, via HASHER.

HASHER est un port : le domaine ne choisit pas l'algorithme, il compare.

# Errors

`locus-artifact-refused' quand les deux diffèrent.  Ce qui prouve ne peut pas
être ce qui est demandé : le hash reçu avec le contenu ne vaut rien, seul celui
qui a été **déclaré avant** l'upload sert de preuve — c'est la règle que W6.a
tient côté serveur, et le client la tient dans le même sens."
  (let ((computed (funcall hasher content)))
    (unless (equal computed declared-hash)
      (signal 'locus-artifact-refused
              (list (format "hash déclaré %s, calculé %s" declared-hash computed))))
    computed))

(defun locus-artifact-open-plan (artifact)
  "Ce qu'il faut faire pour ouvrir ARTIFACT, sans le faire.

Rend une alist : `:open' (t ou nil), `:mode' (`read-only'), `:quarantine',
`:reason'.  Une **décision** rendue plutôt qu'exécutée, pour que le refus
d'exécuter soit testable sans écrire un octet sur le disque.

# Errors

`locus-artifact-refused' quand l'état ne permet pas de servir le contenu : un
artefact `declared' n'a pas encore de contenu, un `rejected' n'en a plus, et
les servir ferait afficher quelque chose dont personne ne répond."
  (let ((state (alist-get :state artifact))
        (filename (alist-get :filename artifact)))
    (unless (locus-artifact-servable-p state)
      (signal 'locus-artifact-refused
              (list (format "un artefact %s ne se sert pas" (locus-artifact-badge state)))))
    (list (cons :open t)
          ;; Toujours en lecture seule, et jamais exécuté : §21.2. Un artefact vient
          ;; d'une exécution non fiable et le client est la machine de l'utilisateur.
          (cons :mode 'read-only)
          (cons :execute nil)
          (cons :quarantine (locus-artifact-suspect-p filename))
          (cons :classification (or (alist-get :classification artifact) 'internal)))))

(provide 'locus-artifact)

;;; locus-artifact.el ends here
