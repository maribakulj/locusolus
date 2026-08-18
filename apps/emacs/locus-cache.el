;;; locus-cache.el --- Cache client, borné et jamais canonique  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; `SPEC.md' §21.3 et §22.2.
;;
;; # « Ne sert pas de source canonique » est une contrainte de type
;;
;; §21.3 le demande, et une note dans la documentation ne l'obtient pas : dès
;; qu'une lecture rend la valeur nue, l'appelant suivant la traite comme un
;; fait.  `locus-cache-get' rend donc une **entrée** — valeur, instant de
;; stockage, fraîcheur — et il n'existe pas d'accesseur qui rende la valeur
;; seule.  Lire le cache oblige à voir de quand date ce qu'on lit.
;;
;; C'est aussi ce que §22.2 exige à l'écran : « dernière synchronisation,
;; cursor, état stale ».  Les trois voyagent avec la donnée plutôt qu'à côté,
;; parce qu'à côté ils se perdent au premier refactoring.
;;
;; # Les secrets ne sont pas détectés, ils sont refusés
;;
;; « Le cache ne contient pas les secrets » ne se tient pas par une heuristique
;; qui fouillerait les valeurs : elle raterait le premier format qu'elle ne
;; connaît pas, tout en donnant l'impression d'avoir vérifié.  L'appelant
;; déclare, et le cache refuse — un refus explicite vaut mieux qu'une détection
;; qui se trompe en silence.

;;; Code:

(require 'cl-lib)
(require 'locus)

(defcustom locus-cache-ttl 3600
  "Durée, en secondes, au-delà de laquelle une entrée est périmée."
  :type 'integer
  :group 'locus)

(defvar locus-cache-clock-function #'float-time
  "Fonction rendant l'instant courant — un port, pour que les tests ne
dépendent pas de l'horloge.

Un test de péremption qui attendrait vraiment une heure ne serait pas exécuté ;
un test qui manipule l'horloge de la machine casserait le reste de la suite.")

(cl-defstruct (locus-cache-entry (:constructor locus-cache--make-entry)
                                 (:copier nil))
  "Ce qu'une lecture rend : jamais la valeur seule."
  value stored-at classification cursor)

(defvar locus-cache--store (make-hash-table :test #'equal)
  "Les entrées, indexées par clé.")

(define-error 'locus-cache-refused "Le cache refuse cette entrée" 'locus-error)

(defun locus-cache-put (key value &rest options)
  "Ranger VALUE sous KEY.

OPTIONS accepte `:sensitive', `:classification' et `:cursor'.

# Errors

`locus-cache-refused' quand l'appelant déclare la valeur sensible.  Le cache
survit à l'arrêt d'Emacs et se copie avec le répertoire : ce qui y entre est
durable, et un secret durable est un secret perdu."
  (when (plist-get options :sensitive)
    (signal 'locus-cache-refused
            (list key "une valeur déclarée sensible ne se met pas en cache")))
  (puthash key
           (locus-cache--make-entry
            :value value
            :stored-at (funcall locus-cache-clock-function)
            :classification (or (plist-get options :classification) 'internal)
            :cursor (plist-get options :cursor))
           locus-cache--store)
  value)

(defun locus-cache-get (key)
  "L'entrée rangée sous KEY, ou nil.

Rend une `locus-cache-entry', pas la valeur : voir le commentaire d'en-tête.
Une entrée périmée est **rendue** avec `locus-cache-stale-p' vrai plutôt que
supprimée — §22.1 autorise la lecture offline, et effacer au premier
dépassement de TTL priverait le mode offline de ce qu'il existe pour montrer."
  (gethash key locus-cache--store))

(defun locus-cache-stale-p (entry)
  "Renvoyer non-nil quand ENTRY a dépassé son TTL."
  (and entry
       (> (- (funcall locus-cache-clock-function)
             (locus-cache-entry-stored-at entry))
          locus-cache-ttl)))

(defun locus-cache-age (entry)
  "L'âge d'ENTRY en secondes — la « dernière synchronisation » de §22.2."
  (and entry
       (- (funcall locus-cache-clock-function)
          (locus-cache-entry-stored-at entry))))

(defun locus-cache-forget (key)
  "Oublier KEY."
  (remhash key locus-cache--store))

(defun locus-cache-purge ()
  "Tout oublier — §21.3 : « peut être purgé par commande »."
  (interactive)
  (clrhash locus-cache--store))

(defun locus-cache-size ()
  "Le nombre d'entrées."
  (hash-table-count locus-cache--store))

(provide 'locus-cache)

;;; locus-cache.el ends here
