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
  "Un daemon qui répond, pour PAYLOAD.

La requête est **retirée** avant la comparaison : le client pagine, donc il
demande `/timeline?limit=500&cursor=…', et une fixture qui comparerait le
chemin entier ne reconnaîtrait plus aucune route.  Un vrai daemon route sur le
chemin et lit la requête à part ; celle-ci fait pareil, sans quoi elle
cesserait d'éprouver ce que le client fait vraiment."
  (let* ((brut (when (string-match "\\`GET \\([^ ]+\\) " payload)
                 (match-string 1 payload)))
         (chemin (and brut (car (split-string brut "?"))))
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
        (if (string-match-p "GET /workers" payload)
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

(ert-deftest locus-session-les-missions-se-lisent-dans-le-journal ()
  "**Ce qu'on vient regarder** : ce qui tourne en ce moment.

Le cockpit affichait le journal brut — une suite d'événements dans laquelle il
faut compter soi-même pour savoir si une mission tourne encore.  C'est le
travail que l'écran est censé faire."
  (let ((journal
         (list '((position . 1) (event_type . "task.proposed") (stream_id . "task/task_A"))
               '((position . 2) (event_type . "task.queued")   (stream_id . "task/task_A"))
               '((position . 3) (event_type . "task.leased")   (stream_id . "task/task_A"))
               '((position . 4) (event_type . "run.started")   (stream_id . "task/task_A"))
               '((position . 5) (event_type . "task.proposed") (stream_id . "task/task_B"))
               ;; Pas une tâche : un worker.  Il ne doit pas devenir une mission.
               '((position . 6) (event_type . "worker.registered") (stream_id . "worker/w1")))))
    (let ((missions (locus-session--missions journal)))
      (should (= (length missions) 2))
      ;; La plus récente d'abord.
      (should (equal (nth 0 (car missions)) "task_B"))
      (should (equal (nth 1 (car missions)) "proposée"))
      ;; Le **dernier** événement d'une tâche gagne, et rien n'est déduit.
      (let ((a (assoc "task_A" missions)))
        (should (equal (nth 1 a) "EN COURS"))
        (should (locus-session--mission-en-cours-p a))))))

(ert-deftest locus-session-une-mission-terminee-cesse-d-etre-en-cours ()
  "Les états terminaux sont **nommés**, pas déduits d'un rang dans la table.

Insérer un état au milieu de `locus-session-mission-states' ne doit pas changer
ce qui compte comme fini : c'est le genre de couplage qui se casse en silence."
  (let ((journal
         (list '((position . 1) (event_type . "run.started")   (stream_id . "task/task_A"))
               '((position . 2) (event_type . "run.completed") (stream_id . "task/task_A")))))
    (let ((mission (car (locus-session--missions journal))))
      (should (equal (nth 1 mission) "terminée"))
      (should-not (locus-session--mission-en-cours-p mission)))))

(ert-deftest locus-session-un-type-inconnu-s-affiche-tel-quel ()
  "Un événement que ce cockpit ne connaît pas ne se range pas dans un état voisin.

La machine à états du daemon évolue plus vite que ce client.  Traduire un type
inconnu en « en cours » ou en « terminée » afficherait une mesure inventée ;
l'afficher tel quel est laid et vrai, et c'est ce qu'on veut du couple."
  (let ((mission (car (locus-session--missions
                       (list '((position . 1) (event_type . "task.exotique")
                               (stream_id . "task/task_Z")))))))
    (should (equal (nth 1 mission) "task.exotique"))
    ;; Inconnu n'est pas terminal : mieux vaut montrer une mission de trop que
    ;; taire une qui tourne.
    (should (locus-session--mission-en-cours-p mission))))

(ert-deftest locus-session-le-direct-ne-survit-pas-a-son-tampon ()
  "Un minuteur qui survivrait au cockpit interrogerait le daemon pour personne.

C'est la fuite la plus difficile à voir : elle ne casse rien, elle consomme, et
rien à l'écran ne dit qu'elle tourne."
  (locus-session-test--reset)
  ;; `unwind-protect' n'est pas une précaution de style : un `should' qui échoue
  ;; ici sortirait en laissant le minuteur armé, et c'est
  ;; `locus-separation-charger-n-arme-aucun-timer' qui rougirait — dans un autre
  ;; fichier, sur une propriété que ce test-ci n'a pas violée.  Un test qui fait
  ;; échouer son voisin coûte plus cher que ce qu'il vérifie.
  (unwind-protect
      (progn
        (locus-cockpit-auto-mode 1)
        (should locus-cockpit--timer)
        (when (get-buffer locus-session--dashboard-buffer)
          (kill-buffer locus-session--dashboard-buffer))
        (locus-cockpit--tick)
        (should-not locus-cockpit-auto-mode)
        (should-not locus-cockpit--timer))
    (locus-cockpit-auto-mode -1)))

(ert-deftest locus-session-le-direct-relit-et-redessine ()
  "Un tour de direct fait les deux moitiés, et le tampon suit l'état du daemon.

C'est la propriété qui distingue un cockpit « en direct » d'un cockpit qu'on
rafraîchit à la main : ce qui change chez le daemon apparaît à l'écran sans que
personne n'ait rien tapé."
  (locus-session-test--reset)
  (unwind-protect
      (locus-session-test--avec #'locus-session-test--repond
        (locus-cockpit)
        (should (get-buffer locus-session--dashboard-buffer))
        ;; Le daemon gagne une mission entre deux tours ; le tour suivant la montre.
        (let ((locus-session-test--reponses
               (cons '("/timeline"
                       . "{\"items\":[{\"position\":9,\"event_type\":\"run.started\",\"stream_id\":\"task/task_NEUVE\"}],\"next\":null}")
                     locus-session-test--reponses)))
          (locus-cockpit--tick))
        (with-current-buffer locus-session--dashboard-buffer
          (let ((texte (buffer-substring-no-properties (point-min) (point-max))))
            (should (string-match-p "task_NEUVE" texte))
            (should (string-match-p "EN COURS" texte))
            (should (string-match-p "1 en cours sur 1" texte)))))
    (locus-cockpit-auto-mode -1)))

(ert-deftest locus-session-une-collection-se-lit-au-dela-de-la-premiere-page ()
  "**Le journal ne s'arrête pas au cinquantième événement.**

Le daemon rend une page et un curseur.  Le cockpit ne lisait que la première :
passé le cinquantième événement il montrait un laboratoire figé au passé, et
l'orchestrateur attendait indéfiniment une fin déjà écrite, hors de sa vue.

Le défaut est invisible tant qu'on essaie — les premières missions tiennent
dans la première page, tout marche, et l'écran cesse de bouger un jour sans que
rien n'ait changé."
  (locus-session-test--reset)
  (let ((demandes nil))
    (locus-session-test--avec
        (lambda (_host _port payload)
          (let ((chemin (when (string-match "\\`GET \\([^ ]+\\) " payload)
                          (match-string 1 payload))))
            (push chemin demandes)
            (cond
             ((string-match-p "cursor=c1" chemin)
              "HTTP/1.1 200 OK\r\n\r\n{\"items\":[\"deuxieme\"],\"next\":null}")
             ((string-match-p "\\`/timeline" chemin)
              "HTTP/1.1 200 OK\r\n\r\n{\"items\":[\"premier\"],\"next\":\"c1\"}")
             (t "HTTP/1.1 200 OK\r\n\r\n{\"items\":[],\"next\":null}"))))
      (let ((page (locus-session--page-entiere "timeline")))
        (should (equal (append (alist-get (quote items) page) nil)
                       (list "premier" "deuxieme")))
        ;; Les deux pages ont bien été demandées, la seconde avec le curseur.
        (should (seq-find (lambda (c) (string-match-p "cursor=c1" c)) demandes))))))

(ert-deftest locus-session-le-suivi-de-curseur-a-une-borne ()
  "Un journal sans fin ne se relit pas en entier toutes les trois secondes.

Et la page rendue **ne prétend pas** être complète : `next' garde le dernier
curseur suivi plutôt que nil, parce que dire qu'il n'y a plus rien alors qu'on
s'est arrêté à la borne serait affirmer une exhaustivité qu'on n'a pas."
  (locus-session-test--reset)
  (let ((appels 0))
    (locus-session-test--avec
        (lambda (&rest _)
          (cl-incf appels)
          "HTTP/1.1 200 OK\r\n\r\n{\"items\":[\"encore\"],\"next\":\"toujours\"}")
      (let* ((locus-session-pages-max 3)
             (page (locus-session--page-entiere "timeline")))
        (should (= appels 3))
        (should (= (length (alist-get (quote items) page)) 3))
        (should (equal (alist-get (quote next) page) "toujours"))))))

(provide 'locus-session-test)

;;; locus-session-test.el ends here
