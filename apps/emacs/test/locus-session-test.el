;;; locus-session-test.el --- Test de sortie de W8.k  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Quelqu'un ouvre le cockpit et voit l'état du laboratoire.**
;;
;; C'est le premier test de sortie de `W8' formulé en termes d'usage plutôt
;; qu'en propriété de composant, et c'est la raison d'être de l'item : les dix
;; précédents étaient tenus, et rien ne s'ouvrait.  Un test qui ne dit pas ce
;; qu'un utilisateur obtient laisse passer exactement ce défaut-là.
;;
;; # Le transport est un port, donc il se remplace
;;
;; `locus-http-send-function' existe pour ça.  Les tests posent une fonction à
;; la place de la socket, et aucun n'a besoin d'un daemon : ce qui est éprouvé
;; ici est l'assemblage, pas le réseau — que `W8.i' a déjà éprouvé de son côté.
;;
;; Les deux tests qui comptent le plus sont ceux de la **panne**, pas ceux du
;; succès.  Un cockpit sert d'abord quand quelque chose ne va pas ; celui qui ne
;; s'ouvrirait que si tout va bien serait inutile au seul moment où on le
;; regarde.

;;; Code:

(require 'ert)
(require 'cl-lib)
(require 'locus)
(require 'locus-cache)
(require 'locus-session)

;; ------------------------------------------------------------------------
;; Un daemon de mensonge, et un daemon éteint
;; ------------------------------------------------------------------------

(defconst locus-session-test--reponses
  '(("/projections/status"
     . "{\"ready\":true,\"projections\":[{\"name\":\"execution_graph\",\"healthy\":true}]}")
    ("/workers"   . "{\"items\":[\"worker-a\",\"worker-b\"],\"next\":null}")
    ("/conflicts" . "{\"items\":[],\"next\":null}")
    ("/timeline"  . "{\"items\":[\"mission.started\"],\"next\":null}"))
  "Ce qu'un daemon rend, par chemin — les formes réelles de `apps/locusd'.")

(defun locus-session-test--repond (_host _port payload)
  "Un daemon qui répond, pour PAYLOAD."
  (let* ((chemin (when (string-match "\\`GET \\([^ ]+\\) " payload)
                   (match-string 1 payload)))
         (corps (or (cdr (assoc chemin locus-session-test--reponses))
                    "{\"items\":[],\"next\":null}")))
    (format "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n%s" corps)))

(defun locus-session-test--eteint (&rest _)
  "Un daemon éteint : la socket ne s'ouvre pas."
  (error "make client process failed: Connection refused, :name, locus-http"))

(defmacro locus-session-test--avec (transport &rest body)
  "Exécuter BODY avec TRANSPORT à la place de la socket."
  (declare (indent 1))
  `(let ((locus-http-send-function ,transport)
         (locus-endpoint "http://127.0.0.1:8787"))
     ,@body))

(defmacro locus-session-test--hors-ligne (&rest body)
  "Exécuter BODY avec toute sortie réseau rendue fatale."
  (declare (indent 0))
  `(cl-letf (((symbol-function 'url-retrieve)
              (lambda (&rest _) (error "le rendu a tenté un url-retrieve")))
             ((symbol-function 'make-network-process)
              (lambda (&rest _) (error "le rendu a ouvert une socket")))
             ((symbol-function 'open-network-stream)
              (lambda (&rest _) (error "le rendu a ouvert un stream"))))
     ,@body))

(defun locus-session-test--reset ()
  "Repartir d'un client qui n'a rien vu."
  (locus-cache-purge)
  (setq locus--connection nil)
  (when (get-buffer "*Locus Solus*")
    (kill-buffer "*Locus Solus*")))

;; ------------------------------------------------------------------------
;; Ce que l'item existe pour tenir
;; ------------------------------------------------------------------------

(ert-deftest locus-session-le-cockpit-montre-l-etat-du-laboratoire ()
  "**Le test de sortie.**  Une commande, et l'état est à l'écran.

Ce que les dix items de `W8' ne disaient nulle part : pas qu'un buffer se
reconstruit, mais que quelqu'un l'obtient en appelant une commande."
  (locus-session-test--reset)
  (locus-session-test--avec #'locus-session-test--repond
    (locus-cockpit)
    (with-current-buffer "*Locus Solus*"
      (let ((texte (buffer-substring-no-properties (point-min) (point-max))))
        (should (locus-connected-p))
        (should (string-match-p "execution_graph" texte))
        (should (string-match-p "worker-a" texte))
        (should (string-match-p "worker-b" texte))
        (should (string-match-p "mission.started" texte))
        (should (string-match-p "connecté" texte))))))

(ert-deftest locus-session-le-cockpit-s-ouvre-sans-daemon ()
  "Un daemon éteint donne un cockpit, pas une erreur.

C'est le moment où on regarde un cockpit.  Celui qui refuserait de s'ouvrir
faute de daemon serait absent précisément quand il sert."
  (locus-session-test--reset)
  (locus-session-test--avec #'locus-session-test--eteint
    ;; N'échoue pas : la commande doit rendre un tampon.
    (locus-cockpit)
    (with-current-buffer "*Locus Solus*"
      (let ((texte (buffer-substring-no-properties (point-min) (point-max))))
        (should-not (locus-connected-p))
        (should (string-match-p "non connecté" texte))
        (should (string-match-p "Connection refused" texte))))))

(ert-deftest locus-session-la-coupure-ne-vide-pas-l-ecran ()
  "Après une coupure, le dernier état connu reste affiché — et daté.

Un écran qui se viderait ferait lire une panne de transport comme un
laboratoire au repos, ce qui est l'erreur que l'ADR 0028 décision 4 nomme."
  (locus-session-test--reset)
  (locus-session-test--avec #'locus-session-test--repond
    (locus-session-connect)
    (locus-session-refresh))
  (locus-session-test--avec #'locus-session-test--eteint
    (let ((echecs (locus-session-refresh)))
      (should (= (length echecs) (length locus-session-collections)))
      (should (= (locus-cache-size) (length locus-session-collections)))
      (with-current-buffer (locus-cockpit-render echecs)
        (let ((texte (buffer-substring-no-properties (point-min) (point-max))))
          ;; Ce qui avait été lu est toujours là…
          (should (string-match-p "worker-a" texte))
          ;; … et l'écran dit que le lien est rompu.
          (should (string-match-p "non connecté" texte))
          (should (string-match-p "il y a [0-9]+s" texte)))))))

(ert-deftest locus-session-jamais-lu-ne-se-lit-pas-comme-vide ()
  "Une collection jamais lue et une collection vide s'écrivent différemment.

`/conflicts' rend une page vide : le laboratoire n'a pas de conflit.  Une
collection absente du cache n'a jamais été demandée.  Les afficher pareil
ferait passer une ignorance pour une mesure — « pas vérifié n'est jamais
réussi », appliqué à l'écran."
  (locus-session-test--reset)
  (locus-session-test--avec #'locus-session-test--repond
    (locus-cache-put "conflicts" (locus-session-get "conflicts")))
  (locus-session-test--hors-ligne
    (with-current-buffer (locus-cockpit-render nil)
      (let ((texte (buffer-substring-no-properties (point-min) (point-max))))
        (should (string-match-p "Conflits\n  (aucun)" texte))
        (should (string-match-p "Workers\n  — jamais lu" texte))))))

(ert-deftest locus-session-le-rendu-ne-parle-a-personne ()
  "Le rendu lit le cache et rien d'autre — `W8.d', étendu à l'écran entier.

Vérifié en empoisonnant les primitives réseau : un rendu qui irait chercher
échoue ici, au lieu de réussir plus lentement."
  (locus-session-test--reset)
  (locus-session-test--avec #'locus-session-test--repond
    (locus-session-connect)
    (locus-session-refresh))
  (locus-session-test--hors-ligne
    (with-current-buffer (locus-cockpit-render nil)
      (should (string-match-p "worker-a"
                              (buffer-substring-no-properties (point-min) (point-max)))))))

(ert-deftest locus-session-un-echec-partiel-ne-declare-pas-la-connexion-perdue ()
  "Une seule route qui répond prouve que le lien tient.

La symétrie du test précédent, et elle compte autant : déclarer la connexion
perdue parce qu'une collection a échoué enverrait chercher un daemon éteint
là où il y a une route en difficulté."
  (locus-session-test--reset)
  (locus-session-test--avec
      (lambda (host port payload)
        (if (string-match-p "GET /workers " payload)
            (error "make client process failed: Connection refused, :name, x")
          (locus-session-test--repond host port payload)))
    (locus-session-connect)
    (let ((echecs (locus-session-refresh)))
      (should (= (length echecs) 1))
      (should (equal (caar echecs) "workers"))
      (should (locus-connected-p)))))

(ert-deftest locus-session-se-deconnecter-ne-perd-pas-ce-qui-etait-su ()
  "`locus-session-disconnect' retire le lien, jamais la mémoire.

Purger le cache confondrait « je ne parle plus au daemon » avec « je n'ai
jamais rien su »."
  (locus-session-test--reset)
  (locus-session-test--avec #'locus-session-test--repond
    (locus-session-connect)
    (locus-session-refresh))
  (locus-session-disconnect)
  (should-not (locus-connected-p))
  (should (= (locus-cache-size) (length locus-session-collections))))

(ert-deftest locus-session-une-erreur-du-serveur-n-est-pas-une-panne-de-transport ()
  "Un 500 se distingue d'une socket fermée — ils envoient chercher ailleurs.

Le premier dit que le daemon est là et a refusé ; le second qu'il n'est pas
là.  Un message qui les confondrait ferait redémarrer un daemon qui tourne."
  (locus-session-test--reset)
  (locus-session-test--avec
      (lambda (&rest _) "HTTP/1.1 500 Internal Server Error\r\n\r\n{}")
    (let ((motif (should-error (locus-session-get "workers")
                               :type 'locus-session-unreachable)))
      (should (string-match-p "répond 500" (format "%s" motif)))
      (should-not (string-match-p "injoignable sur" (format "%s" motif))))))

(ert-deftest locus-session-charger-n-ouvre-aucune-connexion ()
  "La règle de `W8.a' survit à l'ajout d'une session — `SPEC.md' §7.1.

C'est la garantie que cet item avait le plus de chances de casser : un point
d'entrée qui se connecterait au chargement rendrait le démarrage d'Emacs
dépendant d'un daemon.  Le fichier est chargé pour exécuter cette suite ; si
son chargement avait connecté, la variable ne serait pas nil ici."
  (should-not (locus-connected-p)))

(ert-deftest locus-session-un-evenement-se-lit-comme-un-evenement ()
  "Un objet de `/timeline' s'affiche en texte, pas en alist Elisp.

Trouvé à l'écran plutôt qu'en écrivant : la première timeline non vide rendait
`((position . 1) (event_type . worker.registered) (stream_id . worker/…))',
qui est lisible pour qui écrit du Lisp et pour personne d'autre.  Le cas ne
s'était pas présenté tant qu'aucun worker ne s'était enrôlé — une collection
vide ne dit rien du rendu de ses éléments."
  (should (equal (locus-session--rendre-item "worker-a") "worker-a"))
  (let ((rendu (locus-session--rendre-item
                '((position . 1)
                  (event_type . "worker.registered")
                  (stream_id . "worker/canterel-587c")))))
    (should (string-match-p "worker\\.registered" rendu))
    (should (string-match-p "worker/canterel-587c" rendu))
    (should-not (string-match-p "event_type" rendu))
    (should-not (string-match-p "(" rendu))))

(provide 'locus-session-test)

;;; locus-session-test.el ends here
