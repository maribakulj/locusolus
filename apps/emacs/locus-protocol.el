;;; locus-protocol.el --- La version de LEP que ce client parle  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; Une constante, et la raison pour laquelle elle est seule.
;;
;; Le client doit savoir quelle version de LEP il parle.  Il ne doit pas savoir
;; ce que « compatible » veut dire : `docs/06' le définit, `packages/protocol' le
;; met en œuvre, et le réécrire ici en ferait une seconde définition — celle qui
;; dérive.  Le `CLAUDE.md' du dépôt l'interdit sous le nom de « duplication
;; cross-repo des contrats ».
;;
;; La règle de négociation arrivera donc avec le handshake (W8.b), et elle
;; viendra du serveur plutôt que d'une copie locale.  Ce qui reste ici est la
;; seule chose qu'un client doit porter lui-même : ce qu'il annonce.
;;
;; La valeur n'est pas libre non plus.  `test/locus-protocol-test.el' la compare
;; à `schemas/lep/1.0/features.json', qui est la source.  Une constante qu'aucun
;; test ne rattache à sa source est une constante qui vieillit sans qu'on le
;; voie.

;;; Code:

(defconst locus-protocol-version "lep/1.0"
  "Version du protocole LEP annoncée par ce client.

Distincte de `locus-version' : le client évolue plus vite que le protocole.
La valeur est vérifiée contre `schemas/lep/1.0/features.json' par les tests —
c'est le schéma qui décide, pas ce fichier.")

(provide 'locus-protocol)

;;; locus-protocol.el ends here
