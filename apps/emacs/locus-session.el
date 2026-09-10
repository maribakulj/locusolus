;;; locus-session.el --- La session : joindre, rafraîchir, montrer  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; `SPEC.md' §7 et §9.  Ce que les dix items de `W8' ont laissé.
;;
;; # Ce qui manquait, et qui n'était pas du code
;;
;; `W8.a' à `W8.j' ont livré l'authentification, le cache, les curseurs, le
;; transport, les commandes, les artefacts, les projections et le rendu — tous
;; testés, aucun appelé.  Le cockpit exposait quatre commandes interactives, et
;; trois d'entre elles ne parlaient à personne.
;;
;; La cause tient dans la forme des tests de sortie plutôt que dans un oubli :
;; ils énoncent des propriétés de composants — « un buffer se reconstruit depuis
;; le cache sans réseau », « une commande mutante sans `expected_revision' n'est
;; pas constructible ».  Aucun ne dit « quelqu'un ouvre le cockpit et voit
;; l'état du laboratoire ».  On pouvait donc tout cocher sans que rien ne
;; s'assemble, et personne n'a menti.
;;
;; `W20' a fait ce diagnostic pour le côté Rust, dans ces termes : « Tout ce qui
;; précède est une bibliothèque.  Vingt-quatre crates savent décrire […] et rien
;; n'expose quoi que ce soit à un client. »  La phrase valait aussi pour ce
;; paquet-ci.  Ce fichier est sa réponse côté client.
;;
;; # Deux moitiés, et la frontière entre elles est la propriété de W8.d
;;
;; **Joindre** remplit le cache ; **montrer** lit le cache.  La séparation n'est
;; pas une élégance : `W8.d' exige qu'un buffer se reconstruise sans réseau, et
;; sa suite empoisonne les primitives réseau pour le vérifier.  Un rendu qui
;; irait chercher lui-même rendrait l'écran aussi disponible que le daemon,
;; alors qu'il sert précisément à savoir ce qui se passe quand quelque chose ne
;; va pas.
;;
;; La conséquence pratique se voit à l'écran : après une coupure, le cockpit
;; montre toujours le dernier état connu, en disant son âge.  Il ne se vide pas.
;;
;; # Injoignable n'est pas vide
;;
;; ADR 0028 décision 4 le dit du broker, et ça vaut ici : « injoignable » et
;; « rien à montrer » envoient chercher à des endroits opposés.  Un daemon
;; éteint produit donc une ligne qui le nomme, jamais une liste vide qui se
;; lirait comme un laboratoire au repos.

;;; Code:

(require 'cl-lib)
(require 'url-parse)
(require 'locus)
(require 'locus-auth)
(require 'locus-cache)
(require 'locus-http)

(define-error 'locus-session-unreachable
  "Daemon Locus injoignable" 'locus-error)

(defgroup locus-session nil
  "La session du cockpit : ce qui joint le daemon et ce qui l'affiche."
  :group 'locus
  :prefix "locus-session-")

;; --------------------------------------------------------------------------
;; Ce que la session va chercher
;; --------------------------------------------------------------------------

(defconst locus-session-collections
  '(("projections/status" . "projections")
    ("workers"            . "workers")
    ("conflicts"          . "conflicts")
    ("timeline"           . "timeline"))
  "Les routes lues à chaque rafraîchissement, en (CHEMIN . CLÉ-DE-CACHE).

Quatre, et pas les dix-neuf que le daemon sert.  Les autres sont paramétrées —
`branches/{id}/history', `graph/{revision_id}' — donc elles demandent qu'on
sache déjà quoi demander ; les servir ici supposerait un identifiant que le
cockpit n'a pas encore.  Elles arriveront avec la commande qui les nomme.

Ce sont aussi celles qui répondent sans autorisation sur un daemon amorcé sans
administrateur, ce qui est le cas d'un `personal-local' au premier démarrage :
un cockpit qui exigerait une créance pour afficher quoi que ce soit rendrait le
premier lancement impossible à réussir.")

(defconst locus-session--dashboard-buffer "*Locus Solus*"
  "Le tampon du cockpit.

Distinct de `locus-dashboard-buffer-name' — §9.1 réserve ce nom-là au tableau
des **programmes**, dont aucune route ne rend encore la matière.  Les
confondre ferait afficher sous un nom promis par la spec un contenu qui n'est
pas celui qu'elle décrit.")

;; --------------------------------------------------------------------------
;; L'endpoint, relu plutôt que supposé
;; --------------------------------------------------------------------------

(defun locus-session-host-port ()
  "L'hôte et le port de `locus-endpoint', en (HÔTE . PORT).

# Errors

`locus-session-unreachable' quand `locus-endpoint' n'est pas une URL dont on
peut tirer un hôte.  Une adresse illisible se signale ici, où on la lit, et non
trois appels plus loin sous la forme d'une connexion refusée."
  (let* ((url (url-generic-parse-url locus-endpoint))
         (host (url-host url))
         (port (url-portspec url)))
    (unless (and host (not (string-empty-p host)))
      (signal 'locus-session-unreachable
              (list (format "`locus-endpoint' ne porte pas d'hôte : %S" locus-endpoint))))
    (cons host (or port (if (equal (url-type url) "https") 443 80)))))

;; --------------------------------------------------------------------------
;; Joindre — la moitié qui parle
;; --------------------------------------------------------------------------

(defvar locus-session-credential-function nil
  "Une fonction sans argument rendant la créance, ou nil.

Un **port**, et il existe parce que `locus-auth' ne couvre pas tous les cas.
`auth-source' est le bon endroit pour un secret durable — c'est ce que §6.1
demande, et rien ici ne le remet en cause.  Mais un daemon `personal-local'
lancé pour la session courante a une créance qui naît et meurt avec lui :
l'écrire dans `~/.authinfo.gpg' y laisserait, après extinction, une ligne qui
n'ouvre plus rien.

Quand cette variable est nil — le défaut —, `locus-auth' décide seul, et le
comportement est celui d'avant.  L'ordre est donc : le port s'il est posé,
`auth-source' sinon.  Il ne s'inverse pas : une créance éphémère explicitement
fournie est un choix de l'appelant, et une entrée durable ne doit pas la
supplanter en silence.")

(defun locus-session--cause (err)
  "La cause de ERR, sans le déversement d'Emacs.

`make client process failed' rend son message suivi de tout le plist de
connexion — hôte, service, coding, tls — soit huit paires qui répètent ce que
l'appelant vient d'écrire.  Affiché tel quel, l'essentiel (« Connection
refused ») se noie, et quatre collections en échec remplissent l'écran de la
même phrase quatre fois.

La coupure se fait au premier mot-clé : ce qui précède est la cause, ce qui
suit est le contexte que nous connaissons déjà."
  (let ((message (error-message-string err)))
    (if (string-match "\\(.*?\\), :[a-z]" message)
        (match-string 1 message)
      message)))

(defun locus-session-get (path)
  "Lire PATH sur le daemon et rendre le corps relu.

L'autorisation est posée **si** une créance est disponible, et son absence
n'est pas une erreur : un daemon `personal-local' amorcé sans administrateur
sert ses lectures sans en demander, et exiger une créance ici rendrait le
premier démarrage impossible à réussir.

L'en-tête `Host' est ajouté ici plutôt que dans `locus-http-build' : il dépend
de l'endpoint, que la construction ne connaît pas — et HTTP/1.1 le rend
obligatoire, un serveur conforme refusant la requête sans lui.

# Errors

`locus-session-unreachable' quand la socket ne s'ouvre pas, ou quand le serveur
répond une erreur.  Les deux se distinguent par le message : le premier cas
n'a pas de statut, le second en a un."
  (pcase-let* ((`(,host . ,port) (locus-session-host-port))
               (request (locus-http-build
                         "GET" (concat "/" path)
                         :headers (list (cons "Host" (format "%s:%d" host port))
                                        (cons "Connection" "close"))))
               (response (condition-case err
                             (locus-http-send host port (locus-session--authorized request))
                           (locus-http-malformed (signal 'locus-session-unreachable
                                                         (list (error-message-string err))))
                           (error (signal 'locus-session-unreachable
                                          (list (format "%s injoignable sur %s:%d — %s"
                                                        path host port
                                                        (locus-session--cause err))))))))
    (let ((status (alist-get :status response)))
      (unless (and status (>= status 200) (< status 300))
        (signal 'locus-session-unreachable
                (list (format "%s : le daemon répond %s%s" path status
                              (let ((envelope (alist-get :error response)))
                                (if envelope (format " — %S" envelope) ""))))))
      (alist-get :body response))))

(defun locus-session--authorized (request)
  "REQUEST, autorisée quand une créance est disponible.

`locus-auth-authorization' travaille sur une alist et `locus-http-build' rend
une structure : la conversion vit ici, dans le seul endroit qui a besoin des
deux.  La déplacer dans l'un des deux modules lui ferait connaître l'autre, ce
que leur séparation existe pour éviter."
  (let ((creance (and locus-session-credential-function
                      (ignore-errors (funcall locus-session-credential-function)))))
    (cond
     ;; Le port, quand il est posé et qu'il rend quelque chose.
     ((and creance (stringp creance) (not (string-empty-p creance)))
      (setf (locus-http-request-headers request)
            (cons (cons "Authorization" (concat "Bearer " creance))
                  (locus-http-request-headers request)))
      request)
     ((not (locus-auth-available-p)) request)
     (t
      (condition-case nil
          (let* ((porte (list (cons :headers (locus-http-request-headers request))))
                 (signee (locus-auth-authorization porte)))
            (setf (locus-http-request-headers request) (alist-get :headers signee))
            request)
        ;; Une créance présente mais illisible ne doit pas empêcher les lectures
        ;; publiques : le serveur dira lui-même si elle manquait.
        (error request))))))

;;;###autoload
(defun locus-session-connect ()
  "Joindre le daemon et retenir qu'on l'a joint.

Une **action explicite**, jamais un effet de chargement : `SPEC.md' §7.1 veut
que le démarrage d'Emacs tienne sans daemon, et `locus-separation' le vérifie.

La preuve de vie est une lecture réelle de `projections/status' plutôt qu'une
socket ouverte : un port qui accepte ne dit pas qu'un daemon Locus est derrière,
et se déclarer connecté à un autre logiciel serait pire que de se déclarer
déconnecté."
  (interactive)
  (let ((status (locus-session-get "projections/status")))
    (setq locus--connection (list :endpoint locus-endpoint
                                  :at (float-time)))
    (locus-cache-put "projections" status)
    (when (called-interactively-p 'interactive)
      (message "locus : %s — %s" locus-endpoint
               (if (eq (alist-get 'ready status) t)
                   "projections prêtes"
                 "projections en retard")))
    status))

;;;###autoload
(defun locus-session-disconnect ()
  "Oublier la connexion.

Le cache n'est **pas** vidé : ce qui a été lu reste vrai à la date où il l'a
été, et l'écran continue de le montrer en disant son âge.  Purger ici
confondrait « je ne parle plus au daemon » avec « je n'ai jamais rien su »."
  (interactive)
  (setq locus--connection nil)
  (when (called-interactively-p 'interactive)
    (message "locus : déconnecté — le cache est conservé")))

;;;###autoload
(defun locus-session-refresh ()
  "Relire les collections de `locus-session-collections' dans le cache.

Chaque route est lue **indépendamment** : une qui échoue laisse les autres
entrer.  Un rafraîchissement tout-ou-rien ferait perdre trois lectures réussies
parce qu'une quatrième a échoué, et l'écran serait vide là où il pouvait être
partiellement à jour.

Rend la liste des échecs, en (CLÉ . MOTIF) — vide quand tout est passé."
  (interactive)
  (let (echecs)
    (pcase-dolist (`(,path . ,key) locus-session-collections)
      (condition-case err
          (locus-cache-put key (locus-session-get path))
        (locus-session-unreachable
         (push (cons key (error-message-string err)) echecs))))
    (setq echecs (nreverse echecs))
    ;; Toutes les routes en échec, c'est le daemon qui n'est plus là — pas
    ;; quatre pannes indépendantes.  Rester « connecté » afficherait un état
    ;; que rien ne soutient, et l'en-tête du cockpit se lirait comme une
    ;; confirmation.  Une seule qui passe, en revanche, prouve le contraire :
    ;; le lien tient, et c'est cette route-là qui a un problème.
    (when (and locus--connection
               (= (length echecs) (length locus-session-collections)))
      (setq locus--connection nil))
    (when (called-interactively-p 'interactive)
      (message "locus : %d collection(s) à jour%s"
               (- (length locus-session-collections) (length echecs))
               (if echecs (format ", %d en échec" (length echecs)) "")))
    echecs))

;; --------------------------------------------------------------------------
;; Montrer — la moitié qui ne parle à personne
;; --------------------------------------------------------------------------

(defvar-local locus-session--failures nil
  "Les échecs du dernier rafraîchissement, affichés en tête.

Portés par le tampon plutôt que globaux : deux cockpits sur deux endpoints ne
partagent pas leurs pannes.")

(defvar locus-cockpit-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "g") #'locus-cockpit-refresh)
    (define-key map (kbd "d") #'locus-cockpit-auto-mode)
    (define-key map (kbd "c") #'locus-session-connect)
    (define-key map (kbd "q") #'quit-window)
    map)
  "Le clavier du cockpit.")

(define-derived-mode locus-cockpit-mode special-mode "Locus"
  "L'état du laboratoire, reconstruit depuis le cache.

\\{locus-cockpit-mode-map}"
  (setq buffer-read-only t)
  (setq-local revert-buffer-function
              (lambda (&rest _) (locus-cockpit-refresh))))

(defun locus-session--age (key)
  "L'âge de l'entrée KEY, en texte, ou nil si elle n'existe pas."
  (let ((entry (locus-cache-get key)))
    (when entry
      (format "il y a %ds%s"
              (round (locus-cache-age entry))
              (if (locus-cache-stale-p entry) ", PÉRIMÉ" "")))))

(defun locus-session--items (key)
  "Les éléments de la page rangée sous KEY, ou nil.

Le daemon rend `{\"items\":[…],\"next\":…}' : la forme est lue ici, une fois,
plutôt que par chaque section."
  (let ((entry (locus-cache-get key)))
    (when entry
      (let ((page (locus-cache-entry-value entry)))
        (append (alist-get 'items page) nil)))))

(defun locus-session--rendre-item (item)
  "Le texte d'un ITEM de collection.

Le daemon rend deux formes selon la route : `/workers' et `/conflicts' rendent
des chaînes, `/timeline' des objets.  Un rendu unique par `format \"%s\"'
affichait l'alist Elisp telle quelle — parenthèses, points, symboles —, ce qui
est lisible pour qui écrit du Lisp et pour personne d'autre.

Les champs sont nommés dans l'ordre où ils se lisent : ce qui s'est passé,
puis à qui.  Une forme inconnue retombe sur `%s' plutôt que d'être tue : une
route qui gagnerait un champ doit se voir, fût-ce mal."
  (cond
   ((stringp item) item)
   ((and (consp item) (consp (car item)))
    (let ((type (alist-get 'event_type item))
          (flux (alist-get 'stream_id item))
          (rang (alist-get 'position item)))
      (if type
          (format "%s%s%s"
                  (if rang (format "%-4s " rang) "")
                  type
                  (if flux (format "   %s" flux) ""))
        (format "%s" item))))
   (t (format "%s" item))))

(defconst locus-session-mission-states
  '(("task.proposed"  . "proposée")
    ("task.queued"    . "en file")
    ("task.leased"    . "réclamée")
    ("run.started"    . "EN COURS")
    ("run.completed"  . "terminée")
    ("run.failed"     . "ÉCHOUÉE")
    ("attempt.rejected" . "refusée"))
  "Ce qu'un type d'événement dit de l'état d'une mission — §11.2.

Une table plutôt qu'un `cond' : la machine à états du daemon évolue plus vite
que ce cockpit, et un type inconnu doit s'afficher **tel quel** au lieu d'être
rangé de force dans un état voisin.  Un état inventé se lit comme une mesure.")

(defun locus-session--missions (events)
  "L'état de chaque mission, lu du journal EVENTS.

# Le dernier événement gagne, et rien d'autre n'est déduit

Le journal est ordonné par `position' : l'état d'une tâche est celui que dit
son événement le plus récent, point.  Reconstituer une machine à états ici —
« réclamée implique en file » — ferait exister deux vérités sur les mêmes
transitions, et celle du daemon est la seule qui compte.

Rend une liste de (IDENTIFIANT ÉTAT POSITION), les plus récentes d'abord.  Un
journal élagué rend donc moins de missions qu'il n'y en a eu, ce qui est exact :
ce cockpit montre ce que le daemon lui a dit, jamais ce qu'il suppose."
  (let ((par-tache (make-hash-table :test 'equal)))
    (dolist (evt events)
      (when (consp evt)
        (let* ((flux (alist-get 'stream_id evt))
               (type (alist-get 'event_type evt))
               (rang (or (alist-get 'position evt) 0))
               (id (and (stringp flux)
                        (string-prefix-p "task/" flux)
                        (substring flux (length "task/")))))
          (when id
            (let ((connu (gethash id par-tache)))
              (when (or (null connu) (> rang (nth 1 connu)))
                (puthash id (list type rang) par-tache)))))))
    (let (missions)
      (maphash (lambda (id etat)
                 (push (list id
                             (or (cdr (assoc (nth 0 etat) locus-session-mission-states))
                                 (nth 0 etat))
                             (nth 1 etat))
                       missions))
               par-tache)
      (sort missions (lambda (a b) (> (nth 2 a) (nth 2 b)))))))

(defun locus-session--mission-en-cours-p (mission)
  "Vrai quand MISSION n'a pas atteint un état terminal.

Les états terminaux sont nommés, pas déduits d'une position dans la table :
ajouter un état au milieu ne doit pas changer ce qui compte comme fini."
  (not (member (nth 1 mission) '("terminée" "ÉCHOUÉE" "refusée"))))

(defun locus-session--insert-missions ()
  "La section des missions, repliée depuis le journal déjà en cache.

Ne lit rien de plus que `Timeline' — c'est la même page, comptée autrement.
Une requête de plus ferait dépendre cette section du réseau alors que la
donnée est déjà là, et le cockpit sert d'abord quand le réseau ne va pas."
  (let ((entry (locus-cache-get "timeline")))
    (insert (propertize "Missions\n" 'face 'bold))
    (if (null entry)
        (insert "  — jamais lu\n\n")
      (let* ((missions (locus-session--missions (locus-session--items "timeline")))
             (actives (seq-filter #'locus-session--mission-en-cours-p missions)))
        (if (null missions)
            (insert "  (aucune)\n")
          ;; Les actives d'abord et en gras : c'est ce qu'on vient regarder.
          ;; Les terminées restent, parce qu'une mission qui vient de finir est
          ;; l'information qu'on cherche aussi souvent que celle qui tourne.
          (dolist (m missions)
            (let ((en-cours (locus-session--mission-en-cours-p m)))
              (insert "  "
                      (propertize (format "%-10s" (nth 1 m))
                                  'face (if en-cours 'warning 'shadow))
                      " " (nth 0 m) "\n"))))
        (insert (propertize (format "  · %d en cours sur %d · %s\n"
                                    (length actives) (length missions)
                                    (locus-session--age "timeline"))
                            'face 'shadow))))
    (insert "\n")))

(defun locus-session--insert-section (titre key rendu)
  "Insérer la section TITRE pour la collection KEY, chaque élément par RENDU.

Une collection **absente du cache** et une collection **vide** ne s'écrivent
pas pareil : la première n'a jamais été lue, la seconde l'a été et ne contenait
rien.  Les afficher identiquement ferait lire un daemon injoignable comme un
laboratoire au repos."
  (let ((entry (locus-cache-get key)))
    (insert (propertize (format "%s\n" titre) 'face 'bold))
    (cond
     ((null entry)
      (insert "  — jamais lu\n"))
     (t
      (let ((items (locus-session--items key)))
        (if (null items)
            (insert "  (aucun)\n")
          (dolist (item items)
            (insert "  " (funcall rendu item) "\n")))
        (insert (propertize (format "  · %s\n" (locus-session--age key))
                            'face 'shadow)))))
    (insert "\n")))

(defun locus-session--insert-projections ()
  "La section des projections — sa forme n'est pas une page."
  (let ((entry (locus-cache-get "projections")))
    (insert (propertize "Projections\n" 'face 'bold))
    (if (null entry)
        (insert "  — jamais lu\n\n")
      (let* ((value (locus-cache-entry-value entry))
             (ready (eq (alist-get 'ready value) t)))
        (insert (format "  prêtes : %s\n" (if ready "oui" "NON")))
        (dolist (p (append (alist-get 'projections value) nil))
          (insert (format "  %-22s %s\n"
                          (alist-get 'name p)
                          (if (eq (alist-get 'healthy p) t) "saine" "EN RETARD"))))
        (insert (propertize (format "  · %s\n\n" (locus-session--age "projections"))
                            'face 'shadow))))))

(defun locus-cockpit-render (&optional failures)
  "Construire le tampon du cockpit et le rendre.

# Ne parle à personne

La fonction ne lit que le cache — c'est la propriété de `W8.d', étendue à
l'écran entier plutôt qu'au seul tableau des programmes.  FAILURES est ce que
`locus-session-refresh' a rapporté ; il s'affiche en tête, parce qu'un écran
partiellement à jour qui ne le dirait pas se lirait comme un écran à jour."
  (let ((buffer (get-buffer-create locus-session--dashboard-buffer)))
    (with-current-buffer buffer
      (let ((inhibit-read-only t)
            (point-avant (point)))
        (erase-buffer)
        (locus-cockpit-mode)
        (setq locus-session--failures failures)
        (insert (propertize (format "Locus Solus %s — %s\n" locus-version locus-endpoint)
                            'face 'bold))
        (insert (format "protocole %s · %s\n\n"
                        locus-protocol-version
                        (if (locus-connected-p) "connecté" "non connecté")))
        (dolist (echec failures)
          (insert (propertize (format "!! %s : %s\n" (car echec) (cdr echec))
                              'face 'error)))
        (when failures (insert "\n"))
        (locus-session--insert-projections)
        (locus-session--insert-missions)
        (locus-session--insert-section
         "Workers" "workers" #'locus-session--rendre-item)
        (locus-session--insert-section
         "Conflits" "conflicts" #'locus-session--rendre-item)
        (locus-session--insert-section
         "Timeline" "timeline" #'locus-session--rendre-item)
        (insert (propertize
                 (format "g rafraîchir · d direct (%s) · c connecter · q quitter\n"
                         (if (bound-and-true-p locus-cockpit-auto-mode)
                             (format "actif, %ds" locus-cockpit-auto-interval)
                           "arrêté"))
                 'face 'shadow))
        (goto-char (min point-avant (point-max)))))
    buffer))

;;;###autoload
(defun locus-cockpit-refresh ()
  "Relire le daemon, puis redessiner.

Les deux moitiés, dans l'ordre et séparées : si la première échoue en entier,
la seconde dessine quand même le dernier état connu."
  (interactive)
  (let ((echecs (locus-session-refresh)))
    (locus-cockpit-render echecs)))

;;;###autoload
(defun locus-cockpit ()
  "Ouvrir le cockpit : l'état du laboratoire, dans un tampon.

C'est le point d'entrée du client.  Il joint le daemon s'il ne l'est pas
encore, relit les collections, et affiche — mais aucune de ces trois étapes
n'est requise pour que les suivantes aient lieu : un daemon éteint donne un
cockpit qui montre le dernier état connu et nomme la panne, ce qui est
exactement ce dont on a besoin quand quelque chose ne va pas."
  (interactive)
  (unless (locus-connected-p)
    (condition-case err
        (locus-session-connect)
      (locus-session-unreachable
       (message "locus : %s" (error-message-string err)))))
  (pop-to-buffer (locus-cockpit-refresh)))

;; --------------------------------------------------------------------------
;; Le direct — une interrogation régulière, pas un flux
;; --------------------------------------------------------------------------

;; `locusd' ne pousse rien : `/timeline', `/workers' et `/conflicts' se lisent
;; par pages, avec un curseur.  Le « direct » est donc une **interrogation
;; régulière**, et c'est un choix qui se voit plutôt qu'une limite qu'on
;; masque : un cockpit qui prétendrait recevoir un flux tairait le délai entre
;; ce qui s'est passé et ce qu'il montre, alors que ce délai est précisément ce
;; qu'un exploitant doit connaître.

(defcustom locus-cockpit-auto-interval 3
  "Secondes entre deux relectures quand `locus-cockpit-auto-mode' est actif.

Trois secondes, et pas moins : chaque tour lit quatre routes, et un intervalle
plus court ferait travailler le daemon pour montrer un écran qui n'a pas
changé.  Une mission dure des minutes ; la voir avec trois secondes de retard
n'a jamais empêché personne de la piloter."
  :type 'integer
  :group 'locus)

(defvar locus-cockpit--timer nil
  "Le minuteur du mode direct, ou nil.

Un seul pour toute la session : deux cockpits sur deux endpoints partageraient
ce minuteur, et c'est voulu — il rafraîchit le tampon courant du cockpit, qui
est unique par construction (`locus-session--dashboard-buffer').")

(defun locus-cockpit--tick ()
  "Un tour de direct : relire, redessiner, et s'arrêter si le tampon est parti.

# Pourquoi le minuteur se coupe lui-même

Un minuteur qui survivrait à son tampon interrogerait le daemon pour personne,
indéfiniment, et rien à l'écran ne dirait qu'il tourne.  C'est la forme de fuite
la plus difficile à voir : elle ne casse rien, elle consomme."
  (let ((buffer (get-buffer locus-session--dashboard-buffer)))
    (if (not (buffer-live-p buffer))
        (locus-cockpit-auto-mode -1)
      (with-current-buffer buffer
        ;; Le point et le début de fenêtre sont préservés par `locus-cockpit-render' ;
        ;; sans cela, un écran qui se redessine toutes les trois secondes ramènerait
        ;; la lecture en haut à chaque tour, ce qui le rendrait inutilisable
        ;; exactement quand il y a quelque chose à lire.
        (locus-cockpit-refresh)))))

;;;###autoload
(define-minor-mode locus-cockpit-auto-mode
  "Relire le laboratoire toutes les `locus-cockpit-auto-interval' secondes.

Global, parce que le minuteur l'est : le lier à un tampon donnerait un mode
qui semble actif dans une fenêtre et inerte dans une autre, pour un même
minuteur."
  :global t
  :lighter " ⟳"
  :group 'locus
  (when (timerp locus-cockpit--timer)
    (cancel-timer locus-cockpit--timer)
    (setq locus-cockpit--timer nil))
  (when locus-cockpit-auto-mode
    (setq locus-cockpit--timer
          (run-with-timer locus-cockpit-auto-interval
                          locus-cockpit-auto-interval
                          #'locus-cockpit--tick)))
  (when (called-interactively-p 'interactive)
    (message "locus : direct %s"
             (if locus-cockpit-auto-mode
                 (format "activé — relecture toutes les %ds" locus-cockpit-auto-interval)
               "arrêté"))))

(provide 'locus-session)

;;; locus-session.el ends here
