;;; locus-http-test.el --- Test de sortie de W8.i  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Une requête se construit et se relit sans réseau ; l'erreur structurée du
;; serveur arrive comme une erreur structurée ; un aller-retour réel passe.**
;;
;; Les deux premiers tiers sont purs, et c'est le point : les fautes de
;; transport vivent dans la construction et la relecture — en-tête mal formé,
;; corps mal cadré, statut mal interprété — et s'éprouvent au cas par cas.  Le
;; dernier tiers monte un vrai serveur, sur `localhost', et fait un vrai
;; aller-retour : sans lui, le module serait vérifié partout sauf là où il
;; touche le monde.

;;; Code:

(require 'ert)
(require 'cl-lib)
(require 'locus-http)

;; ------------------------------------------------------------------------
;; Construire, sans réseau
;; ------------------------------------------------------------------------

(ert-deftest locus-http-une-requete-se-construit-sans-rien-ouvrir ()
  "La construction est pure : elle ne parle à personne, donc elle se teste au
cas par cas plutôt qu'à travers une socket."
  (let ((request (locus-http-build "get" "/v1/programs")))
    (should (equal (locus-http-request-method request) "GET"))
    (should (equal (locus-http-request-path request) "/v1/programs"))
    (should (null (locus-http-request-body request)))
    (should (null (process-list)))))

(ert-deftest locus-http-un-corps-json-porte-son-type-et-sa-longueur ()
  "Une longueur fausse fait attendre le serveur ou tronque le corps ; elle est
donc comptée en **octets**, pas en caractères — c'est la faute qu'un accent
révèle et qu'un test en ASCII rate."
  (let* ((request (locus-http-build "post" "/v1/commands"
                                    :body '((name . "évaluation"))))
         (headers (locus-http-request-headers request))
         (body (locus-http-request-body request)))
    (should (equal (alist-get "Content-Type" headers nil nil #'equal) "application/json"))
    (should (equal (alist-get "Content-Length" headers nil nil #'equal)
                   (number-to-string (string-bytes body))))
    (ert-info ("l'accent occupe deux octets : la longueur dépasse le nombre de caractères")
      (should (> (string-bytes body) (length body))))))

(ert-deftest locus-http-une-cle-mot-cle-est-refusee-pas-convertie ()
  "`json-serialize' rend le mot-clé `:a' comme `\":a\"' — **avec le
deux-points** — c'est-à-dire un champ que le serveur ne reconnaîtra jamais.
L'échec se manifesterait par un 400 énigmatique, loin d'ici.

Refusé plutôt que converti : convertir supposerait une correspondance entre
les mots-clés d'Elisp et les noms du fil, et cette correspondance serait une
seconde définition du protocole — celle qui dérive."
  (should-error (locus-http-build "post" "/v1/x" :body '((:expected-revision . 7)))
                :type 'locus-http-malformed)
  (ert-info ("le symbole simple passe, et donne le nom du champ tel quel")
    (should (equal (locus-http-request-body
                    (locus-http-build "post" "/v1/x" :body '((expected_revision . 7))))
                   "{\"expected_revision\":7}"))))

(ert-deftest locus-http-la-cle-d-idempotence-voyage-sur-le-fil ()
  "§11.4 : sans elle sur le fil, le serveur ne peut pas dédupliquer, et
l'idempotence du client ne vaut que pour lui-même."
  (let ((headers (locus-http-request-headers
                  (locus-http-build "post" "/v1/commands" :idempotency-key "idem-7"))))
    (should (equal (alist-get "Idempotency-Key" headers nil nil #'equal) "idem-7"))))

(ert-deftest locus-http-une-requete-sans-corps-n-annonce-pas-de-longueur ()
  "Annoncer `Content-Length: 0' sur un GET fait attendre un corps qui ne vient
pas à des serveurs pointilleux."
  (let ((headers (locus-http-request-headers (locus-http-build "get" "/v1/health"))))
    (should-not (alist-get "Content-Length" headers nil nil #'equal))
    (should-not (alist-get "Content-Type" headers nil nil #'equal))))

(ert-deftest locus-http-le-texte-rendu-est-du-http ()
  "Ce qui part se lit, donc se teste."
  (let ((text (locus-http-render (locus-http-build "post" "/v1/x" :body '((a . 1))))))
    (should (string-prefix-p "POST /v1/x HTTP/1.1\r\n" text))
    (should (string-match-p "\r\n\r\n" text))
    (should (string-suffix-p "{\"a\":1}" text))))

;; ------------------------------------------------------------------------
;; Relire, sans réseau
;; ------------------------------------------------------------------------

(ert-deftest locus-http-une-reponse-se-relit-corps-compris ()
  (let ((response (locus-http-parse
                   "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}")))
    (should (equal (alist-get :status response) 200))
    (should (equal (alist-get 'ok (alist-get :body response)) t))
    (ert-info ("les noms d'en-tête sont normalisés : le serveur choisit sa casse, pas nous")
      (should (equal (alist-get "content-type" (alist-get :headers response) nil nil #'equal)
                     "application/json")))))

(ert-deftest locus-http-une-reponse-illisible-n-est-pas-une-reponse-vide ()
  "Les traiter pareil ferait passer une panne de transport pour un résultat."
  (should-error (locus-http-parse "") :type 'locus-http-malformed)
  (should-error (locus-http-parse "bonjour") :type 'locus-http-malformed))

(ert-deftest locus-http-un-corps-illisible-n-est-pas-une-panne-de-transport ()
  "Le serveur a répondu ; c'est le contenu qui surprend.  Les confondre ferait
réessayer une requête qui a abouti."
  (let ((response (locus-http-parse "HTTP/1.1 200 OK\r\n\r\npas du json")))
    (should (equal (alist-get :status response) 200))
    (should (equal (alist-get :body response) "pas du json"))))

(ert-deftest locus-http-un-corps-vide-se-distingue-d-un-corps-absent ()
  (let ((response (locus-http-parse "HTTP/1.1 204 No Content\r\n\r\n")))
    (should (equal (alist-get :status response) 204))
    (should (null (alist-get :body response)))))

;; ------------------------------------------------------------------------
;; L'erreur structurée n'est pas un code
;; ------------------------------------------------------------------------

(ert-deftest locus-http-l-erreur-structuree-arrive-entiere ()
  "`packages/protocol' fait de l'erreur une enveloppe — catégorie, code,
politique de reprise.  Rendre « 409 » jetterait tout cela pour garder le seul
chiffre, et l'appelant devrait deviner s'il peut réessayer."
  (let* ((raw (concat "HTTP/1.1 409 Conflict\r\nContent-Type: application/json\r\n\r\n"
                      "{\"error\":{\"code\":\"revision_conflict\",\"category\":\"conflict\","
                      "\"retryable\":false,\"message\":\"la révision a changé\"}}"))
         (response (locus-http-parse raw))
         (envelope (alist-get :error response)))
    (should envelope)
    (should (equal (alist-get 'code envelope) "revision_conflict"))
    (should (equal (alist-get 'category envelope) "conflict"))
    (should (equal (alist-get 'message envelope) "la révision a changé"))))

(ert-deftest locus-http-la-reprise-se-lit-dans-l-enveloppe-pas-dans-le-statut ()
  "Un 409 de conflit de révision ne se réessaie jamais (§11.3) ; un 409 de
verrou temporaire se réessaie.  Le chiffre ne les distingue pas — seule
l'enveloppe le dit."
  (let ((conflit (locus-http-parse
                  (concat "HTTP/1.1 409 Conflict\r\n\r\n"
                          "{\"error\":{\"code\":\"revision_conflict\",\"retryable\":false}}")))
        (verrou (locus-http-parse
                 (concat "HTTP/1.1 409 Conflict\r\n\r\n"
                         "{\"error\":{\"code\":\"lock_held\",\"retryable\":true}}"))))
    (should (equal (alist-get :status conflit) (alist-get :status verrou)))
    (should-not (locus-http-retryable-p conflit))
    (should (locus-http-retryable-p verrou))))

(ert-deftest locus-http-une-erreur-sans-enveloppe-ne-s-en-invente-pas-une ()
  "Un serveur qui rend une erreur nue ne dit rien de la reprise : supposer
qu'elle est possible ferait boucler sur une faute définitive."
  (let ((response (locus-http-parse "HTTP/1.1 500 Internal Server Error\r\n\r\n")))
    (should (null (alist-get :error response)))
    (should-not (locus-http-retryable-p response))))

(ert-deftest locus-http-un-succes-ne-porte-aucune-enveloppe-d-erreur ()
  (let ((response (locus-http-parse "HTTP/1.1 200 OK\r\n\r\n{\"error\":\"champ métier\"}")))
    (should (null (alist-get :error response)))))

;; ------------------------------------------------------------------------
;; Un aller-retour réel
;; ------------------------------------------------------------------------

(defun locus-http-test--cleanup ()
  "Supprimer tout processus laissé par ces tests.

Un serveur supprimé laisse ses **connexions acceptées** derrière lui, et
plusieurs tests du paquet affirment qu'aucun processus ne tourne — cette suite
les a fait échouer au premier essai.  Le nettoyage porte donc sur la
descendance, pas seulement sur le serveur."
  (dolist (process (process-list))
    (when (string-prefix-p "locus-http" (process-name process))
      (delete-process process))))

(defun locus-http-test--serve (reply)
  "Monter un serveur local qui répond REPLY, et rendre (PROCESS . PORT)."
  (let* ((server (make-network-process
                  :name "locus-http-test" :server t :host 'local :service t
                  :family 'ipv4 :coding 'binary
                  :filter (lambda (connection _chunk)
                            (process-send-string connection reply)
                            (process-send-eof connection))))
         (port (cadr (process-contact server))))
    (cons server port)))

(ert-deftest locus-http-un-aller-retour-reel-passe ()
  "Sans ce test, le module serait vérifié partout sauf là où il touche le
monde : la construction et la relecture sont pures, mais la socket ne l'est
pas, et c'est elle qui manque quand tout le reste est vert."
  (let* ((reply (concat "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n"
                        "{\"programs\":[\"Riemann\"]}"))
         (served (locus-http-test--serve reply))
         (server (car served))
         (port (cdr served)))
    (unwind-protect
        (let ((response (locus-http-send "127.0.0.1" port
                                         (locus-http-build "get" "/v1/programs"))))
          (should (equal (alist-get :status response) 200))
          (should (equal (alist-get 'programs (alist-get :body response)) ["Riemann"])))
      (locus-http-test--cleanup))))

(ert-deftest locus-http-le-serveur-recoit-bien-ce-qui-a-ete-construit ()
  "L'aller compte autant que le retour : une requête bien construite et mal
envoyée est indistinguable, côté client, d'une requête mal construite."
  (let* ((seen "")
         (server (make-network-process
                  :name "locus-http-echo" :server t :host 'local :service t
                  :family 'ipv4 :coding 'binary
                  :filter (lambda (connection chunk)
                            (setq seen (concat seen chunk))
                            (process-send-string connection "HTTP/1.1 200 OK\r\n\r\n")
                            (process-send-eof connection))))
         (port (cadr (process-contact server))))
    (unwind-protect
        (progn
          (locus-http-send "127.0.0.1" port
                           (locus-http-build "post" "/v1/commands"
                                             :body '((type . "branch.rename"))
                                             :idempotency-key "idem-9"))
          (should (string-prefix-p "POST /v1/commands HTTP/1.1" seen))
          (should (string-match-p "Idempotency-Key: idem-9" seen))
          (should (string-match-p "branch.rename" seen)))
      (locus-http-test--cleanup))))

(ert-deftest locus-http-la-socket-est-un-port ()
  "Tout le module est pur sauf une fonction : c'est ce qui permet d'éprouver le
reste sans réseau, et de n'avoir qu'un seul endroit à regarder quand le réseau
est en cause."
  (let* ((vu nil)
         (locus-http-send-function
          (lambda (host port payload)
            (setq vu (list host port payload))
            "HTTP/1.1 200 OK\r\n\r\n")))
    (locus-http-send "exemple.test" 7420 (locus-http-build "get" "/v1/health"))
    (should (equal (nth 0 vu) "exemple.test"))
    (should (equal (nth 1 vu) 7420))
    (should (string-prefix-p "GET /v1/health" (nth 2 vu)))))

(ert-deftest locus-http-le-transport-n-ajoute-aucune-autorisation ()
  "La séparation tient parce que le secret ne doit exister que le temps d'un
appel : l'ajouter au transport le ferait vivre aussi longtemps que la
connexion.  C'est `locus-auth-authorization' qui le pose sur la requête."
  (let ((headers (locus-http-request-headers (locus-http-build "get" "/v1/health"))))
    (should-not (alist-get "Authorization" headers nil nil #'equal))))

(provide 'locus-http-test)

;;; locus-http-test.el ends here
