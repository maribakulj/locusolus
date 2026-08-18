;;; locus-http.el --- Le transport : construire, relire, et une seule socket  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; Ce que les sept items précédents avaient déclaré en écart.
;;
;; # Trois responsabilités, séparées exprès
;;
;; **Construire** une requête, **relire** une réponse, et **parler** à une
;; socket.  Les deux premières sont pures : c'est là que vivent les fautes — un
;; en-tête mal formé, un corps mal cadré, un statut mal interprété — et c'est là
;; qu'on peut les éprouver au cas par cas, sans serveur, donc sans dépendance à
;; une machine.  La troisième est une fonction, isolée derrière un port, et elle
;; ne décide de rien.
;;
;; Un client qui mélangerait les trois se testerait à travers une socket, ce qui
;; veut dire : lentement, par intermittence, et jamais sur le cas rare.
;;
;; # L'erreur structurée n'est pas un code
;;
;; `packages/protocol` fait de l'erreur une **enveloppe** — catégorie, code,
;; politique de reprise, corrélation.  Un client qui rendrait « HTTP 409 »
;; jetterait tout cela pour garder le seul chiffre, et l'appelant devrait
;; deviner s'il peut réessayer.  La relecture reconstitue donc l'enveloppe quand
;; le serveur en envoie une, et dit clairement quand il n'en envoie pas.

;;; Code:

(require 'cl-lib)
(require 'json)
(require 'locus)
(require 'locus-auth)

(define-error 'locus-http-malformed "Réponse HTTP mal formée" 'locus-error)

(defcustom locus-http-timeout 30
  "Secondes avant d'abandonner une requête.

Bornée parce qu'une requête sans limite bloque un Emacs sans rien dire, et que
« lent » et « perdu » se ressemblent exactement du point de vue de qui attend."
  :type 'integer
  :group 'locus)

(cl-defstruct (locus-http-request (:constructor locus-http--make-request) (:copier nil))
  "Une requête, construite et pas encore envoyée."
  method path headers body)

(defun locus-http-build (method path &rest options)
  "Construire une requête pour METHOD et PATH.

OPTIONS accepte `:headers', `:body' et `:idempotency-key'.

Les clés de `:body' sont des symboles simples, écrits comme le champ part sur
le fil — `(expected_revision . 7)', pas `(:expected-revision . 7)'.

Ne parle à personne : la construction est pure, et c'est ce qui permet de
l'éprouver au cas par cas."
  (let* ((body (plist-get options :body))
         (encoded (and body (progn (locus-http--check-keys body) (json-serialize body))))
         (key (plist-get options :idempotency-key))
         (headers (append
                   (plist-get options :headers)
                   (list (cons "Accept" "application/json"))
                   (when encoded
                     (list (cons "Content-Type" "application/json")
                           (cons "Content-Length"
                                 (number-to-string (string-bytes encoded)))))
                   ;; §11.4 : la clé voyage sur le fil, sinon le serveur ne peut
                   ;; pas dédupliquer et l'idempotence du client ne vaut que pour
                   ;; lui-même.
                   (when key (list (cons "Idempotency-Key" key))))))
    (locus-http--make-request :method (upcase method) :path path
                              :headers headers :body encoded)))

(defun locus-http--check-keys (body)
  "Refuser les clés mot-clé dans BODY.

`json-serialize\' rend le mot-clé `:a\' comme `\":a\"' — **avec le
deux-points** — c\'est-à-dire un champ que le serveur ne reconnaîtra jamais.
L\'échec se manifesterait par un 400 énigmatique, loin d\'ici.

Le refus est préféré à une conversion : convertir supposerait une
correspondance entre les mots-clés d\'Elisp et les noms de champs du fil, et
cette correspondance serait une seconde définition du protocole — celle qui
dérive.  L\'appelant écrit donc les noms du fil, tels quels.

# Errors

`locus-http-malformed\' quand une clé est un mot-clé."
  (cond
   ((and (consp body) (consp (car body)))
    (dolist (pair body)
      (when (keywordp (car pair))
        (signal 'locus-http-malformed
                (list (format "clé mot-clé %S : écrivez le nom du champ tel qu\'il part sur le fil"
                              (car pair)))))
      (locus-http--check-keys (cdr pair))))
   ((and (consp body) (keywordp (car body)))
    (signal 'locus-http-malformed
            (list (format "clé mot-clé %S : écrivez le nom du champ tel qu\'il part sur le fil"
                          (car body)))))))

(defun locus-http-render (request)
  "Le texte HTTP de REQUEST, corps compris.

Rendu séparément de l'envoi : ce qui part se lit, donc se teste."
  (concat (format "%s %s HTTP/1.1\r\n"
                  (locus-http-request-method request)
                  (locus-http-request-path request))
          (mapconcat (lambda (header) (format "%s: %s\r\n" (car header) (cdr header)))
                     (locus-http-request-headers request)
                     "")
          "\r\n"
          (or (locus-http-request-body request) "")))

(defun locus-http-parse (raw)
  "Relire la réponse RAW.

Rend une alist : `:status', `:headers', `:body' (décodé quand c'est du JSON),
`:error' (l'enveloppe structurée du serveur, s'il y en a une).

# Errors

`locus-http-malformed' quand la ligne de statut manque ou n'est pas lisible.
Une réponse qu'on ne sait pas lire n'est pas une réponse vide : traiter les
deux pareil ferait passer une panne de transport pour un résultat."
  (let* ((split (string-search "\r\n\r\n" raw))
         (head (if split (substring raw 0 split) raw))
         (body (if split (substring raw (+ split 4)) ""))
         (lines (split-string head "\r\n" t))
         (status-line (car lines)))
    (unless (and status-line (string-match "\\`HTTP/[0-9.]+ \\([0-9]\\{3\\}\\)" status-line))
      (signal 'locus-http-malformed
              (list (format "ligne de statut illisible : %S"
                            (or status-line "(rien)")))))
    (let* ((status (string-to-number (match-string 1 status-line)))
           (headers (locus-http--headers (cdr lines)))
           (decoded (locus-http--decode body)))
      (list (cons :status status)
            (cons :headers headers)
            (cons :body decoded)
            (cons :error (locus-http--structured-error status decoded))))))

(defun locus-http--headers (lines)
  "Les en-têtes portés par LINES, noms en minuscules."
  (delq nil
        (mapcar (lambda (line)
                  (when (string-match "\\`\\([^:]+\\): ?\\(.*\\)\\'" line)
                    (cons (downcase (match-string 1 line)) (match-string 2 line))))
                lines)))

(defun locus-http--decode (body)
  "BODY décodé si c'est du JSON, sinon tel quel.

Un corps illisible n'est pas une erreur de transport : le serveur a répondu, et
c'est le contenu qui surprend.  Les confondre ferait réessayer une requête qui
a abouti."
  (if (string-empty-p (string-trim body))
      nil
    (condition-case nil
        (json-parse-string body :object-type 'alist :null-object nil)
      (error body))))

(defun locus-http--structured-error (status body)
  "L'enveloppe d'erreur portée par BODY, ou nil.

§26 de la spec Canterel fait de l'erreur une enveloppe — catégorie, code,
politique de reprise.  Rendre « 409 » jetterait tout cela pour garder le seul
chiffre, et l'appelant devrait deviner s'il peut réessayer."
  (and (>= status 400)
       (listp body)
       (alist-get 'error body)))

(defun locus-http-retryable-p (response)
  "Renvoyer non-nil quand RESPONSE dit qu'on peut réessayer.

Lit ce que le serveur **déclare**, et ne le déduit pas du statut : un 409 de
conflit de révision ne se réessaie jamais (§11.3) alors qu'un 409 de verrou
temporaire se réessaie, et le chiffre ne les distingue pas."
  (let ((envelope (alist-get :error response)))
    (and envelope (eq (alist-get 'retryable envelope) t))))

(defvar locus-http-send-function #'locus-http--send-over-socket
  "La fonction qui parle à la socket — un port.

Elle reçoit hôte, port et texte de requête, et rend le texte de réponse.  Tout
le reste du module est pur : c'est ce qui permet d'éprouver la construction et
la relecture sans réseau, et de n'avoir qu'un seul endroit à regarder quand le
réseau est en cause.")

(defun locus-http--send-over-socket (host port payload)
  "Envoyer PAYLOAD à HOST:PORT et rendre la réponse brute."
  (let ((process (open-network-stream "locus-http" nil host port))
        (response ""))
    (unwind-protect
        (progn
          (set-process-coding-system process 'binary 'binary)
          (set-process-filter process
                              (lambda (_process chunk)
                                (setq response (concat response chunk))))
          (process-send-string process payload)
          (with-timeout (locus-http-timeout)
            (while (process-live-p process)
              (accept-process-output process 0.05)))
          response)
      (when (process-live-p process) (delete-process process)))))

(defun locus-http-send (host port request)
  "Envoyer REQUEST à HOST:PORT et rendre la réponse relue.

L'autorisation n'est **pas** ajoutée ici : `locus-auth-authorization' la pose
sur la requête, et la séparation tient parce que le secret ne doit exister que
le temps d'un appel — l'ajouter au transport le ferait vivre aussi longtemps
que la connexion."
  (locus-http-parse
   (funcall locus-http-send-function host port (locus-http-render request))))

(provide 'locus-http)

;;; locus-http.el ends here
