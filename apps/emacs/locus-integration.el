;;; locus-integration.el --- Les dépendances optionnelles, uniformément  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; `SPEC.md' §4.3, appliqué aux intégrations de §15 à §20.
;;
;; # Un mécanisme, pas six
;;
;; §4.3 pose quatre règles pour **chaque** dépendance optionnelle : détectée,
;; commandes ajoutées seulement si disponible, erreur actionnable si appelée
;; sans elle, et rien de cassé au démarrage.  Écrites six fois — Org, Magit,
;; xiiif, Jupyter, `eat', Denote — elles seraient tenues cinq fois et demie :
;; c'est la sixième qu'on découvre en production, sur la machine de quelqu'un
;; qui n'a pas installé le paquet.
;;
;; # Détecter n'est pas charger
;;
;; La règle la moins évidente, et celle qui décide de la forme du module.
;; `(require 'magit nil t)' détecte parfaitement — et charge Magit.  Le démarrage
;; d'Emacs paierait alors toutes les dépendances optionnelles du cockpit, ce que
;; §7.1 interdit (« ne pas ralentir l'ouverture de la première frame ») et ce
;; qui rendrait « optionnel » synonyme de « chargé quand même ».
;;
;; La détection regarde donc si la bibliothèque **existe**, sans l'évaluer.

;;; Code:

(require 'cl-lib)
(require 'locus)

(define-error 'locus-integration-missing
              "Dépendance optionnelle absente"
              'locus-error)

(defvar locus-integration--registry (make-hash-table :test #'eq)
  "Les intégrations déclarées, par nom.")

(cl-defstruct (locus-integration (:constructor locus-integration--make) (:copier nil))
  "Une intégration facultative.

Le champ s'appelle `provides' et non `commands' : `cl-defstruct' engendre
l'accesseur `locus-integration-provides', ce qui laisse le nom
`locus-integration-commands' à la fonction publique — celle qui ne rend rien
quand la dépendance manque.  Deux noms proches pour deux sens différents
auraient fini par être confondus à l'appel."
  name feature package provides)

(defun locus-integration-forget-all ()
  "Oublier toutes les intégrations déclarées."
  (clrhash locus-integration--registry))

(defun locus-integration-declare (name feature &rest options)
  "Déclarer l'intégration NAME, qui repose sur FEATURE.

OPTIONS accepte `:package' (le nom sous lequel l'utilisateur l'installera, s'il
diffère de FEATURE) et `:commands' (la liste des commandes qu'elle ajoute).

Déclarer **ne charge rien** et ne vérifie rien : une déclaration qui sonderait
le disque ferait payer au démarrage autant que les `require' qu'elle remplace."
  (puthash name
           (locus-integration--make
            :name name
            :feature feature
            :package (or (plist-get options :package) feature)
            :provides (plist-get options :commands))
           locus-integration--registry)
  name)

(defun locus-integration-get (name)
  "L'intégration NAME, ou nil."
  (gethash name locus-integration--registry))

(defun locus-integration-names ()
  "Les noms déclarés, triés."
  (sort (hash-table-keys locus-integration--registry)
        (lambda (a b) (string< (symbol-name a) (symbol-name b)))))

(defun locus-integration-available-p (name)
  "Renvoyer non-nil quand l'intégration NAME peut servir.

# Détecter n'est pas charger

`featurep' pour ce qui est déjà là, `locate-library' pour ce qui est
installable — et rien qui évalue.  Employer `require' ici détecterait aussi
bien et chargerait la dépendance, ce qui ferait payer au démarrage tout ce que
« facultatif » est censé épargner."
  (let ((integration (locus-integration-get name)))
    (and integration
         (let ((feature (locus-integration-feature integration)))
           (or (featurep feature)
               (and (locate-library (symbol-name feature)) t))))))

(defun locus-integration-commands (name)
  "Les commandes que NAME ajoute, ou nil quand elle n'est pas disponible.

§4.3 : « ajoute ses commandes seulement si disponible ».  Une commande offerte
puis défaillante est pire qu'une commande absente : elle se découvre au moment
où on en a besoin."
  (and (locus-integration-available-p name)
       (locus-integration-commands-declared name)))

(defun locus-integration-commands-declared (name)
  "Les commandes déclarées pour NAME, disponibles ou non.

Distincte de `locus-integration-commands' : celle-ci dit ce que l'intégration
apporterait, celle-là ce qu'elle apporte ici et maintenant.  La première sert à
documenter, la seconde à construire un menu."
  (let ((integration (locus-integration-get name)))
    (and integration (locus-integration-provides integration))))

(defun locus-integration-require (name)
  "Exiger l'intégration NAME, et rendre sa `feature'.

À appeler depuis une commande, au moment où elle sert — pas au chargement.

# Errors

`locus-integration-missing' quand la dépendance est absente, avec le nom du
paquet à installer.  §4.3 exige une erreur **actionnable** : un message qui dit
seulement « indisponible » oblige à lire le code pour savoir quoi installer, et
c'est le moment où l'utilisateur en a le moins envie."
  (let ((integration (locus-integration-get name)))
    (unless integration
      (signal 'locus-integration-missing
              (list (format "intégration inconnue : %s" name))))
    (unless (locus-integration-available-p name)
      (signal 'locus-integration-missing
              (list (format "« %s » demande le paquet `%s', qui n'est pas installé"
                            name (locus-integration-package integration)))))
    (let ((feature (locus-integration-feature integration)))
      (require feature)
      feature)))

(provide 'locus-integration)

;;; locus-integration.el ends here
