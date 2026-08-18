;;; locus-events-test.el --- Test de sortie de W8.c  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Une déconnexion ne perd ni ne duplique un événement.**
;;
;; Les deux moitiés se testent ensemble ou pas du tout : un flux qui refuse
;; tout ne duplique rien, et un flux qui accepte tout ne perd rien.  Le
;; scénario central coupe au milieu, reprend avec le chevauchement que le
;; serveur rejoue, et vérifie les deux propriétés sur la **même** trace.

;;; Code:

(require 'ert)
(require 'cl-lib)
(require 'locus-events)

(defun locus-events-test--event (sequence &optional kind)
  "Un événement de rang SEQUENCE, de sorte KIND."
  (list (cons :seq sequence)
        (cons :kind (or kind "progress"))
        (cons :id (format "evt-%03d" sequence))))

(defun locus-events-test--sequences (stream)
  "Les rangs gardés par STREAM, dans l'ordre."
  (mapcar (lambda (event) (alist-get :seq event))
          (locus-event-stream-events stream)))

;; ------------------------------------------------------------------------
;; Le scénario qui porte le sprint
;; ------------------------------------------------------------------------

(ert-deftest locus-events-une-deconnexion-ne-perd-ni-ne-duplique ()
  "Dix événements, coupure après le cinquième, reprise avec chevauchement.

Le serveur rejoue à partir du curseur demandé : les rangs 4 et 5 reviennent, et
c'est normal — c'est ce chevauchement qui garantit qu'aucun n'a été sauté. Le
client doit donc les reconnaître sans les recompter."
  (let ((stream (locus-events-make-stream)))
    (dotimes (index 5)
      (should (eq (locus-events-accept stream (locus-events-test--event (1+ index)))
                  'accepted)))
    (should (equal (locus-events-resume-from stream) 5))

    (locus-events-disconnected stream)
    (should (locus-event-stream-stale stream))

    ;; Reprise : le serveur renvoie depuis 4, avec recouvrement.
    (locus-events-reconnected stream)
    (should-not (locus-event-stream-stale stream))
    (dolist (sequence '(4 5 6 7 8 9 10))
      (locus-events-accept stream (locus-events-test--event sequence)))

    ;; Rien de perdu, rien de dupliqué : les deux moitiés sur la même trace.
    (ert-info ("rien de perdu, rien de dupliqué")
      (should (equal (locus-events-test--sequences stream) '(1 2 3 4 5 6 7 8 9 10))))
    (should (null (locus-event-stream-gaps stream)))
    (should (equal (locus-events-resume-from stream) 10))))

(ert-deftest locus-events-le-chevauchement-est-un-doublon-pas-une-erreur ()
  "Distinguer les deux est ce qui rend les journaux lisibles : confondus, une
reconnexion réussie ressemblerait à une avarie, et on cesserait de les lire."
  (let ((stream (locus-events-make-stream)))
    (should (eq (locus-events-accept stream (locus-events-test--event 1)) 'accepted))
    (should (eq (locus-events-accept stream (locus-events-test--event 1)) 'duplicate))
    (should (eq (locus-events-accept stream '((:kind . "progress"))) 'malformed))
    (should (equal (locus-events-test--sequences stream) '(1)))))

(ert-deftest locus-events-un-evenement-anterieur-au-curseur-ne-revient-pas ()
  "La déduplication est par curseur, donc exacte et à mémoire constante : un
rang déjà passé est déjà traité, quelle que soit son ancienneté. Une fenêtre
d'identifiants récents aurait une limite au-delà de laquelle un doublon
redeviendrait neuf — un défaut qui ne se manifeste que sur les longues
coupures, les seules où il compte."
  (let ((stream (locus-events-make-stream)))
    (dotimes (index 1000)
      (locus-events-accept stream (locus-events-test--event (1+ index))))
    (should (eq (locus-events-accept stream (locus-events-test--event 1)) 'duplicate))
    (should (equal (locus-events-resume-from stream) 1000))))

;; ------------------------------------------------------------------------
;; Ce qui manque est marqué
;; ------------------------------------------------------------------------

(ert-deftest locus-events-un-trou-est-marque-pas-tu ()
  "§14.1 : « marque les gaps ».  Recevoir 8 quand on en était à 5 n'est pas une
erreur de transport — le serveur peut avoir élagué — mais le taire ferait
passer un historique troué pour un historique complet."
  (let ((stream (locus-events-make-stream)))
    (locus-events-accept stream (locus-events-test--event 5))
    (locus-events-accept stream (locus-events-test--event 8))
    (should (equal (locus-event-stream-gaps stream) '((6 . 7))))))

(ert-deftest locus-events-un-flux-continu-ne-marque-aucun-trou ()
  "Sans ce cas, « marquer les trous » pourrait vouloir dire « marquer toujours »."
  (let ((stream (locus-events-make-stream)))
    (dolist (sequence '(1 2 3 4))
      (locus-events-accept stream (locus-events-test--event sequence)))
    (should (null (locus-event-stream-gaps stream)))))

(ert-deftest locus-events-le-premier-evenement-n-est-pas-un-trou ()
  "Commencer au rang 42 — parce que le serveur a élagué avant, ou parce qu'on
reprend un flux ancien — n'est pas un trou : il n'y avait pas de curseur avant."
  (let ((stream (locus-events-make-stream)))
    (locus-events-accept stream (locus-events-test--event 42))
    (should (null (locus-event-stream-gaps stream)))
    (should (equal (locus-events-resume-from stream) 42))))

(ert-deftest locus-events-une-reconnexion-ne-comble-pas-les-trous ()
  "Elle permet de les demander, elle ne les efface pas.  Un `stale' qui
nettoierait au passage rendrait un historique troué indiscernable d'un
historique complet — la faute même que le marquage existe pour empêcher."
  (let ((stream (locus-events-make-stream)))
    (locus-events-accept stream (locus-events-test--event 1))
    (locus-events-accept stream (locus-events-test--event 5))
    (locus-events-disconnected stream)
    (locus-events-reconnected stream)
    (should (equal (locus-event-stream-gaps stream) '((2 . 4))))))

;; ------------------------------------------------------------------------
;; Le tampon garde ce qui compte
;; ------------------------------------------------------------------------

(ert-deftest locus-events-l-elagage-emporte-le-plus-ancien-ordinaire ()
  "Un tampon borné élague, sinon il n'est pas borné."
  (let ((locus-event-buffer-size 3)
        (stream (locus-events-make-stream)))
    (dotimes (index 5)
      (locus-events-accept stream (locus-events-test--event (1+ index))))
    (should (equal (locus-events-test--sequences stream) '(3 4 5)))))

(ert-deftest locus-events-l-elagage-n-emporte-jamais-un-critique ()
  "§14.1 : « conserve les événements critiques ».

Un tampon qui élague le plus ancien perd les alertes en premier quand le flux
s'emballe — exactement quand elles arrivent.  Le tampon peut donc dépasser sa
taille nominale : dépasser une limite d'affichage est un désagrément, perdre
une alerte de sécurité n'en est pas un."
  (let ((locus-event-buffer-size 2)
        (stream (locus-events-make-stream)))
    (locus-events-accept stream (locus-events-test--event 1 "security"))
    (locus-events-accept stream (locus-events-test--event 2))
    (locus-events-accept stream (locus-events-test--event 3))
    (locus-events-accept stream (locus-events-test--event 4))
    (locus-events-accept stream (locus-events-test--event 5))

    (should (member 1 (locus-events-test--sequences stream)))
    (ert-info ("les ordinaires les plus anciens partent, le critique reste")
      (should (equal (locus-events-test--sequences stream) '(1 4 5))))))

(ert-deftest locus-events-les-sortes-critiques-viennent-de_14_2 ()
  "La liste n'est pas inventée : ce sont les notifications que §14.2 rend par
défaut — celles qui appellent une action et dont l'absence ne se remarque pas."
  (dolist (kind '("security" "approval-requested" "budget-exhausted"
                  "worker-lost" "conflict"))
    (should (locus-events-critical-p (locus-events-test--event 1 kind))))
  (should-not (locus-events-critical-p (locus-events-test--event 1 "progress"))))

;; ------------------------------------------------------------------------
;; Le backoff
;; ------------------------------------------------------------------------

(ert-deftest locus-events-le-backoff-croit-et-reste-borne ()
  "§7.5 : « backoff avec jitter ».  Sans plafond, un backoff exponentiel finit
par ne plus reconnecter du tout, ce qui est indistinguable d'un client cassé."
  (let ((locus-events-jitter-function (lambda () 0.0)))
    (should (= (locus-events-backoff 1) 1.0))
    (should (= (locus-events-backoff 2) 2.0))
    (should (= (locus-events-backoff 3) 4.0))
    (ert-info ("le plafond tient, même très loin")
      (should (= (locus-events-backoff 50) locus-events-backoff-ceiling)))))

(ert-deftest locus-events-le-jitter-ne-rend-jamais-zero ()
  "Un backoff qui peut rendre zéro n'est pas un backoff : sous jitter maximal,
la flotte entière reviendrait immédiatement, ce qui reproduit la panne qu'on
attendait."
  (dolist (jitter '(0.0 0.5 0.999))
    (let ((locus-events-jitter-function (lambda () jitter)))
      (dotimes (attempt 10)
        (let ((delay (locus-events-backoff (1+ attempt))))
          (should (> delay 0))
          (should (<= delay locus-events-backoff-ceiling)))))))

(ert-deftest locus-events-le-jitter-desynchronise-vraiment ()
  "Deux clients qui tirent des jitters différents ne reviennent pas ensemble.
Sans cette propriété, le jitter serait un ornement."
  (let ((early (let ((locus-events-jitter-function (lambda () 0.9)))
                 (locus-events-backoff 4)))
        (late (let ((locus-events-jitter-function (lambda () 0.1)))
                (locus-events-backoff 4))))
    (should (< early late))))

(ert-deftest locus-events-le-jitter-est-un-port ()
  "Il est aléatoire en production et fixé en test : une suite qui dépendrait de
`random' ne serait pas rejouable, et un test de backoff qui échoue une fois sur
vingt finit par être ignoré."
  (should (functionp locus-events-jitter-function))
  (let ((locus-events-jitter-function (lambda () 0.25)))
    (should (= (locus-events-backoff 3) (- 4.0 (* 4.0 0.5 0.25))))))

(provide 'locus-events-test)

;;; locus-events-test.el ends here
