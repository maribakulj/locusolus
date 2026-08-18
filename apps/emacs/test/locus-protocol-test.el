;;; locus-protocol-test.el --- La version annoncée vient du schéma  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; Le client annonce une version de protocole.  Ce test vérifie qu'elle est
;; celle de `schemas/lep/1.0/features.json' — **en la lisant**, pas en la
;; recopiant.  Une constante comparée à une autre constante ne dit rien : les
;; deux vieillissent ensemble, et le test reste vert le jour où le protocole
;; bouge.

;;; Code:

(require 'ert)
(require 'json)
(require 'locus-protocol)

(defconst locus-protocol-test--repository-root
  ;; Capturée **au chargement**, où `load-file-name' désigne ce fichier.  Un
  ;; calcul fait à l'exécution retomberait sur un repli d'une autre profondeur,
  ;; et pointerait à côté sans que rien ne le dise — c'est exactement ce qui est
  ;; arrivé au premier essai.
  (expand-file-name "../../../" (file-name-directory load-file-name))
  "La racine du dépôt, déduite de l'emplacement de ce fichier.

Déduite plutôt que passée en variable d'environnement : un test qui dépend
d'une variable passe silencieusement quand elle est absente.")

(ert-deftest locus-protocol-la-version-annoncee-est-celle-du-schema ()
  "La source est `features.json' ; ce fichier n'en est que le porteur."
  (let* ((features (expand-file-name "schemas/lep/1.0/features.json"
                                     locus-protocol-test--repository-root))
         (declared (with-temp-buffer
                     (insert-file-contents features)
                     (alist-get 'protocol (json-parse-buffer :object-type 'alist)))))
    (should (stringp declared))
    (should (equal locus-protocol-version declared))))

(ert-deftest locus-protocol-le-client-ne-decide-pas-de-la-compatibilite ()
  "Le client annonce, il n'arbitre pas.

`docs/06' définit ce que « compatible » veut dire et `packages/protocol' le met
en œuvre.  Une seconde définition en Elisp serait celle qui dérive — c'est la
« duplication cross-repo des contrats » que le `CLAUDE.md' du dépôt interdit.
Ce test le fixe : aucune fonction de comparaison de versions n'existe ici."
  (should-not (fboundp 'locus-protocol-compatible-p))
  (should-not (fboundp 'locus-protocol-negotiate)))

(provide 'locus-protocol-test)

;;; locus-protocol-test.el ends here
