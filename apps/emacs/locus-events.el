;;; locus-events.el --- Flux d'événements, curseur et reprise  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; `SPEC.md' §14.1 et §7.5.  Une déconnexion ne perd ni ne duplique un
;; événement.
;;
;; # Pourquoi le curseur suffit, et pourquoi il ne suffit pas
;;
;; Dédupliquer par curseur est exact et tient en mémoire constante : un
;; événement dont le rang est déjà passé est déjà traité, point.  Une fenêtre
;; d'identifiants récents serait plus souple et introduirait une limite au-delà
;; de laquelle un doublon redeviendrait neuf — c'est-à-dire un bug qui ne se
;; manifeste que sur les longues coupures, les seules où il compte.
;;
;; Ce que le curseur ne dit pas, c'est ce qui **manque**.  Recevoir le rang 8
;; quand on en était à 5 n'est pas une erreur de transport : le serveur peut
;; avoir élagué, la reprise peut être partielle.  Mais le taire ferait passer un
;; historique troué pour un historique complet, et c'est la seule faute de ce
;; module qu'on ne pourrait plus détecter après coup.  D'où `locus-events-gaps'.
;;
;; # Le tampon garde ce qui compte quand il déborde
;;
;; §14.1 : « conserve les événements critiques ».  Un tampon borné qui élague
;; simplement le plus ancien perd les alertes en premier lorsque le flux
;; s'emballe — exactement quand elles arrivent.  L'élagage épargne donc les
;; critiques, et le tampon peut dépasser sa taille nominale s'ils sont
;; nombreux : dépasser une limite d'affichage est un désagrément, perdre une
;; alerte de sécurité n'en est pas un.

;;; Code:

(require 'cl-lib)
(require 'locus)

(defcustom locus-event-buffer-size 10000
  "Nombre d'événements ordinaires que le flux garde en mémoire — `SPEC.md' §5.

Les événements critiques ne comptent pas dans cette limite : voir
`locus-events-critical-p'."
  :type 'integer
  :group 'locus)

(defcustom locus-events-backoff-base 1.0
  "Délai initial de reconnexion, en secondes — `SPEC.md' §7.5."
  :type 'number
  :group 'locus)

(defcustom locus-events-backoff-ceiling 60.0
  "Délai maximal de reconnexion, en secondes.

Plafonné parce qu'un backoff exponentiel non borné finit par ne plus
reconnecter du tout, ce qui est indistinguable d'un client cassé."
  :type 'number
  :group 'locus)

(defvar locus-events-jitter-function #'locus-events--default-jitter
  "Fonction rendant un flottant dans [0, 1) — un port, pour que les tests
soient rejouables.

Le jitter existe pour désynchroniser les clients qui reconnectent ensemble
après une panne serveur ; il est donc aléatoire en production et fixé en test.
Un backoff sans jitter fait revenir toute la flotte à la même seconde, ce qui
reproduit la panne qu'on attendait.")

(defun locus-events--default-jitter ()
  "Un flottant dans [0, 1)."
  (/ (float (random 1000)) 1000.0))

(cl-defstruct (locus-event-stream (:constructor locus-events-make-stream)
                                  (:copier nil))
  "L'état d'un flux : jusqu'où on a lu, ce qui manque, ce qu'on garde."
  (cursor nil :documentation "Rang du dernier événement accepté, ou nil.")
  (events nil :documentation "Les événements gardés, du plus ancien au plus récent.")
  (gaps nil :documentation "Les intervalles manquants, du plus ancien au plus récent.")
  (stale nil :documentation "Vrai pendant une coupure — `SPEC.md' §7.5."))

(defconst locus-events-critical-kinds
  '("security" "approval-requested" "budget-exhausted" "worker-lost" "conflict")
  "Les sortes d'événements que l'élagage n'emporte jamais.

Tirées des notifications par défaut de §14.2 — celles qui appellent une action
et dont l'absence ne se remarque pas.")

(defun locus-events-critical-p (event)
  "Renvoyer non-nil quand EVENT ne doit pas être élagué."
  (member (alist-get :kind event) locus-events-critical-kinds))

(defun locus-events-accept (stream event)
  "Intégrer EVENT à STREAM ; renvoyer `accepted', `duplicate' ou `malformed'.

# Ce que la valeur de retour sert à distinguer

Un doublon n'est pas une erreur : c'est le fonctionnement normal d'une reprise,
puisque le serveur rejoue depuis le curseur demandé et que le chevauchement est
voulu.  Les confondre ferait journaliser une avarie à chaque reconnexion
réussie, et on finirait par ne plus lire ces journaux."
  (let ((sequence (alist-get :seq event)))
    (cond
     ((not (integerp sequence)) 'malformed)
     ((and (locus-event-stream-cursor stream)
           (<= sequence (locus-event-stream-cursor stream)))
      'duplicate)
     (t
      (locus-events--record-gap stream sequence)
      (setf (locus-event-stream-cursor stream) sequence)
      (locus-events--append stream event)
      'accepted))))

(defun locus-events--record-gap (stream sequence)
  "Noter le trou entre le curseur de STREAM et SEQUENCE, s'il y en a un."
  (let ((cursor (locus-event-stream-cursor stream)))
    (when (and cursor (> sequence (1+ cursor)))
      (setf (locus-event-stream-gaps stream)
            (append (locus-event-stream-gaps stream)
                    (list (cons (1+ cursor) (1- sequence))))))))

(defun locus-events--append (stream event)
  "Ajouter EVENT à STREAM et élaguer sans emporter les critiques."
  (setf (locus-event-stream-events stream)
        (append (locus-event-stream-events stream) (list event)))
  (let ((ordinary (cl-count-if-not #'locus-events-critical-p
                                   (locus-event-stream-events stream))))
    (while (> ordinary locus-event-buffer-size)
      (let ((victim (cl-find-if-not #'locus-events-critical-p
                                    (locus-event-stream-events stream))))
        (setf (locus-event-stream-events stream)
              (delq victim (locus-event-stream-events stream)))
        (setq ordinary (1- ordinary))))))

(defun locus-events-resume-from (stream)
  "Le rang à partir duquel demander la suite — `SPEC.md' §14.1.

Renvoie le curseur, c'est-à-dire le dernier rang **traité** : le serveur reprend
après lui.  Demander `cursor + 1' ferait porter au client une arithmétique qui
appartient au serveur, et un décalage d'un rang y perdrait silencieusement un
événement."
  (locus-event-stream-cursor stream))

(defun locus-events-disconnected (stream)
  "Marquer STREAM comme coupé — l'indicateur `stale' de §7.5."
  (setf (locus-event-stream-stale stream) t)
  stream)

(defun locus-events-reconnected (stream)
  "Marquer STREAM comme rétabli.

Ne touche ni au curseur ni aux trous : une reconnexion ne comble pas ce qui
manquait, elle permet seulement de le demander."
  (setf (locus-event-stream-stale stream) nil)
  stream)

(defun locus-events-backoff (attempt)
  "Le délai avant la tentative de reconnexion ATTEMPT — `SPEC.md' §7.5.

Exponentiel, plafonné, et bruité : le jitter porte sur la moitié haute du
délai, de sorte que le résultat reste dans [base × 2^n / 2, base × 2^n] sans
jamais tomber à zéro. Un backoff qui peut rendre zéro n'est pas un backoff."
  (let* ((raw (* locus-events-backoff-base (expt 2 (max 0 (1- attempt)))))
         (capped (min raw locus-events-backoff-ceiling))
         (jitter (funcall locus-events-jitter-function)))
    (- capped (* capped 0.5 jitter))))

(provide 'locus-events)

;;; locus-events.el ends here
