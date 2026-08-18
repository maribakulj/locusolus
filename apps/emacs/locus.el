;;; locus.el --- Cockpit Emacs pour Locus Solus  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus

;; Author: Locus Solus
;; Version: 0.1.0
;; Package-Requires: ((emacs "30.1"))
;; Keywords: tools, processes
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; Point d'entrée du client Emacs produit de Locus Solus (ADR 0009).
;;
;; Ce fichier est délibérément petit.  Il porte la seule chose que W8.a avait à
;; établir : la frontière.  `apps/emacs' se charge sous `emacs -Q' avec sa seule
;; `load-path', n'ouvre aucune connexion, ne lance aucun processus et n'arme
;; aucun timer.  Charger le client ne doit rien coûter à un Emacs qui démarre.
;;
;; La règle vient de deux endroits qui disent la même chose.  `SPEC.md' §7.1 :
;; « ne pas ralentir l'ouverture de la première frame », « ne lancer aucun stack
;; serveur sans action explicite ».  Et le `CLAUDE.md' de `emacs-config' :
;; « le startup Emacs reste fonctionnel sans réseau et sans que Locus tourne ».
;;
;; Ce qui n'est pas ici y arrivera avec son lecteur.  Les options publiques de
;; `SPEC.md' §5 — `locus-endpoint', `locus-auto-connect', et les autres — ne sont
;; pas déclarées tant qu'aucun code ne les lit : une option que personne ne
;; consulte est une promesse d'API que rien ne tient.

;;; Code:

(require 'locus-protocol)

(defgroup locus nil
  "Cockpit Emacs pour Locus Solus."
  :group 'tools
  :prefix "locus-"
  :link '(url-link "https://github.com/maribakulj/locusolus"))

(defconst locus-version "0.1.0"
  "Version de ce client.

`SPEC.md' §4.4 : « le package publie sa propre version ».  Elle est distincte
de `locus-protocol-version' — le client évolue plus vite que le protocole, et
les confondre rendrait une incompatibilité de protocole illisible.")

(defun locus-connected-p ()
  "Renvoyer non-nil quand le client est connecté à un daemon.

Renvoie toujours nil à ce stade, et c'est une réponse, pas un manque : rien
dans ce package n'ouvre de connexion.  La fonction existe pour que le reste du
cockpit ait un seul endroit où poser la question."
  nil)

;;;###autoload
(defun locus-describe ()
  "Afficher ce qu'est ce client et ce qu'il parle.

Sans réseau : la commande n'interroge aucun daemon.  C'est ce qui la rend utile
quand rien ne répond — dire ce que le client est reste possible même quand dire
ce que le serveur est ne l'est pas."
  (interactive)
  (message "Locus Solus %s, protocole %s, %s"
           locus-version
           locus-protocol-version
           (if (locus-connected-p) "connecté" "non connecté")))

(provide 'locus)

;;; locus.el ends here
