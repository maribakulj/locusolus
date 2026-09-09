;;; locus-mission-test.el --- Test de sortie de W8.l  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Quelqu'un écrit une question dans Emacs, et un worker peut la réclamer.**
;;
;; Le pendant de `W8.k' pour la moitié qui commande.  Formulé en usage, pour la
;; même raison : `locus-command.el' était livré et testé depuis `W8.e', et rien
;; ne l'appelait.  Un test qui vérifie qu'une commande *se construit* laisse
;; passer un daemon que personne ne commande.
;;
;; # Ce que ces tests éprouvent, et ce qu'ils ne peuvent pas éprouver
;;
;; Ils éprouvent ce qui **part** : les trois requêtes, leurs corps, leur ordre,
;; et le fait qu'un refus du serveur arrive à l'appelant comme un refus et non
;; comme un succès muet.  Le transport est remplacé, comme dans `W8.k'.
;;
;; Ils n'éprouvent pas qu'une mission s'exécute : cela demande un hôte capable
;; de confinement, et `W12.e`/`W18.f` sont reportés faute d'un tel hôte.  Ce
;; qui est vérifié ici s'arrête donc exactement là où la roadmap s'arrête, et
;; c'est délibéré — un test qui simulerait l'exécution affirmerait ce que
;; personne n'a mesuré.

;;; Code:

(require 'ert)
(require 'cl-lib)
(require 'locus)
(require 'locus-session)
(require 'locus-mission)

(defvar locus-mission-test--envois nil
  "Les requêtes parties, dans l'ordre, en (CHEMIN . CORPS-JSON).")

(defun locus-mission-test--daemon (_host _port payload)
  "Un daemon qui accepte, et qui retient ce qu'on lui a envoyé."
  (let* ((chemin (when (string-match "\\`POST \\([^ ]+\\) " payload)
                   (match-string 1 payload)))
         (corps (when (string-match "\r\n\r\n\\(.*\\)\\'" payload)
                  (match-string 1 payload))))
    (push (cons chemin corps) locus-mission-test--envois)
    (if (equal chemin "/commands/context-view/build")
        (concat "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\n\r\n"
                "{\"view\":{\"id\":\"ctx_TEST\",\"hash\":\"h4sh\"}}")
      "HTTP/1.1 202 Accepted\r\n\r\n")))

(defmacro locus-mission-test--avec (transport &rest body)
  "Exécuter BODY avec TRANSPORT à la place de la socket."
  (declare (indent 1))
  `(let ((locus-http-send-function ,transport)
         (locus-endpoint "http://127.0.0.1:8787")
         (locus-mission-project "prj_TEST")
         (locus-mission-test--envois nil))
     ,@body))

(defun locus-mission-test--corps (chemin)
  "Le corps relu de la requête envoyée sur CHEMIN.

# Pourquoi le détour par les octets

`json-serialize' rend une chaîne **unibyte** — un accent y occupe déjà ses
deux octets — et `locus-http-render' la concatène à des en-têtes multibytes.
La chaîne rendue porte donc ces octets comme deux caractères distincts, et
`json-parse-string' appliqué tel quel y voit une séquence UTF-8 invalide.

Ce n'est pas un défaut du transport : la socket est en `binary' des deux
côtés, chaque caractère de 0 à 255 part comme son octet, et `Content-Length'
est compté en octets sur la chaîne unibyte.  Ce que la vraie socket fait, ce
test doit donc le faire aussi — repasser aux octets, puis décoder — au lieu de
lire la chaîne comme du texte qu'elle n'est pas encore."
  (let ((brut (cdr (assoc chemin (reverse locus-mission-test--envois)))))
    (and brut
         (json-parse-string (decode-coding-string (string-to-unibyte brut) 'utf-8)
                            :object-type 'alist :null-object nil))))

;; ------------------------------------------------------------------------

(ert-deftest locus-mission-une-question-devient-une-tache-reclamable ()
  "**Le test de sortie.**  Trois requêtes, dans l'ordre, et une tâche en file.

La vue de contexte d'abord : la mission ne porte jamais son contexte, seulement
sa référence et son empreinte, et proposer avant de bâtir citerait une vue qui
n'existe pas."
  (locus-mission-test--avec #'locus-mission-test--daemon
    (let ((id (locus-mission-lancer "La question ?" '("une réponse") "frontier")))
      (should (string-prefix-p "task_" id))
      (should (equal (mapcar #'car (reverse locus-mission-test--envois))
                     '("/commands/context-view/build"
                       "/commands/task/propose"
                       "/commands/task/queue")))
      (let ((propose (locus-mission-test--corps "/commands/task/propose")))
        (let-alist propose
          (should (equal .proposal.statement "La question ?"))
          (should (equal .proposal.cognition "frontier"))
          (should (equal .proposal.task_id id))
          ;; La vue bâtie est celle que la proposition cite, avec son empreinte.
          (should (equal .proposal.context_view_id "ctx_TEST"))
          (should (equal .proposal.context_view_hash "h4sh"))))
      (should (equal (alist-get 'task_id (locus-mission-test--corps
                                          "/commands/task/queue"))
                     id)))))

(ert-deftest locus-mission-chaque-commande-porte-son-projet ()
  "Sans projet, un fait n'a pas d'endroit où appartenir — et le daemon refuse.

Le motif est celui qu'il rend lui-même.  Le test le tient sur les **trois**
commandes : en oublier une ferait échouer la troisième après que les deux
premières ont écrit, ce qui laisse un graphe à moitié fait."
  (locus-mission-test--avec #'locus-mission-test--daemon
    (locus-mission-lancer "Q" '("R") "economy")
    (dolist (chemin '("/commands/context-view/build"
                      "/commands/task/propose"
                      "/commands/task/queue"))
      (should (equal (alist-get 'project_id (locus-mission-test--corps chemin))
                     "prj_TEST")))))

(ert-deftest locus-mission-chaque-commande-porte-sa-cle-d-idempotence ()
  "§22.5 : la clé voyage sur le fil, sinon le serveur ne peut pas dédupliquer.

Et elle **diffère** d'une commande à l'autre : la même clé sur les trois ferait
prendre la mise en file pour une resoumission de la proposition."
  (locus-mission-test--avec #'locus-mission-test--daemon
    (locus-mission-lancer "Q" '("R") "economy")
    (let ((cles (mapcar (lambda (chemin)
                          (alist-get 'idempotency_key
                                     (locus-mission-test--corps chemin)))
                        '("/commands/context-view/build"
                          "/commands/task/propose"
                          "/commands/task/queue"))))
      (should (cl-every (lambda (cle) (string-prefix-p "cmd_" cle)) cles))
      (should (= (length (delete-dups (copy-sequence cles))) 3)))))

(ert-deftest locus-mission-un-refus-du-serveur-arrive-comme-un-refus ()
  "Un 400 ne devient pas un succès muet, et son enveloppe survit.

C'est ce qui a fait trouver le champ `project_id' manquant du premier coup :
le daemon nomme la famille et le détail, et les jeter pour garder le chiffre
obligerait à deviner."
  (locus-mission-test--avec
      (lambda (&rest _)
        (concat "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\n\r\n"
                "{\"error\":{\"family\":\"validation\",\"detail\":\"« project_id » manque\"}}"))
    (let ((motif (should-error (locus-mission-propose "Q" '("R") "frontier")
                               :type 'locus-mission-refused)))
      (should (string-match-p "400" (format "%s" motif)))
      (should (string-match-p "validation" (format "%s" motif))))))

(ert-deftest locus-mission-une-mission-sans-question-est-refusee-avant-le-reseau ()
  "Le refus vient d'ici, pas du serveur — et rien ne part.

Une question vide est une erreur de saisie ; l'envoyer ferait payer un
aller-retour pour apprendre ce qu'on savait déjà, et écrirait une vue de
contexte pour une mission qui n'existera pas."
  (locus-mission-test--avec
      (lambda (&rest _) (error "le réseau ne devait pas être touché"))
    (should-error (locus-mission-propose "   " '("R") "frontier") :type 'user-error)
    (should-error (locus-mission-propose "Q" nil "frontier") :type 'user-error)
    (should (null locus-mission-test--envois))))

(ert-deftest locus-mission-un-identifiant-est-lisible-par-le-daemon ()
  "26 caractères, alphabet de Crockford, premier au plus 7.

Vérifié ici parce qu'un identifiant mal formé n'échoue qu'au serveur, dans un
refus qui parle de parsing et pas de génération."
  (dolist (prefixe '("task" "br" "prj" "ctx"))
    (let* ((id (locus-mission--id prefixe))
           (corps (substring id (1+ (length prefixe)))))
      (should (string-prefix-p (concat prefixe "_") id))
      (should (= (length corps) 26))
      (should (string-match-p "\\`[0-7][0-9A-HJKMNP-TV-Z]\\{25\\}\\'" corps)))))

(provide 'locus-mission-test)

;;; locus-mission-test.el ends here
