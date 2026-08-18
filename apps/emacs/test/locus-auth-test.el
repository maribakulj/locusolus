;;; locus-auth-test.el --- Test de sortie de W8.b  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Aucun secret hors `auth-source' ; une identité absente est une erreur
;; actionnable, pas un plantage.**
;;
;; Les tests emploient un `auth-source' factice plutôt qu'un vrai netrc : lire
;; le fichier de la machine ferait dépendre la suite de ce que l'auteur a
;; configuré, ce que le `CLAUDE.md' du dépôt interdit sous le nom de
;; « dépendance implicite à une machine de développeur ».

;;; Code:

(require 'ert)
(require 'locus-auth)

(defconst locus-auth-test--secret "s3cr3t-de-test"
  "Le credential factice.  Distinctif exprès : les tests le cherchent partout
où il ne doit pas être, et une valeur banale y serait introuvable.")

(defmacro locus-auth-test--with-credential (&rest body)
  "Exécuter BODY avec un `auth-source' qui rend une entrée."
  (declare (indent 0))
  `(cl-letf (((symbol-function 'auth-source-search)
              (lambda (&rest _)
                (list (list :host locus-auth-host
                            :user "marcel"
                            :secret (lambda () locus-auth-test--secret))))))
     ,@body))

(defmacro locus-auth-test--without-credential (&rest body)
  "Exécuter BODY avec un `auth-source' vide."
  (declare (indent 0))
  `(cl-letf (((symbol-function 'auth-source-search) (lambda (&rest _) nil)))
     ,@body))

(require 'cl-lib)

;; ------------------------------------------------------------------------
;; Une identité absente est une erreur actionnable
;; ------------------------------------------------------------------------

(ert-deftest locus-auth-une-identite-absente-est-une-erreur-nommee ()
  "Pas un `wrong-type-argument' sur un nil, pas un backtrace : une condition à
soi, que l'appelant peut rattraper pour proposer le geste suivant."
  (locus-auth-test--without-credential
    (should-error (locus-auth-call-with-credential #'identity)
                  :type 'locus-auth-missing)))

(ert-deftest locus-auth-le-message-contient-le-geste-suivant ()
  "« Actionnable » a un sens vérifiable : le message nomme le fichier et donne
la ligne à y écrire.  Une erreur qui dit seulement ce qui manque oblige à lire
le code pour savoir quoi faire."
  (locus-auth-test--without-credential
    (let ((message (cadr (should-error (locus-auth-call-with-credential #'identity)
                                       :type 'locus-auth-missing))))
      (should (string-match-p "authinfo" message))
      (should (string-match-p (regexp-quote locus-auth-host) message))
      (should (string-match-p "machine" message)))))

(ert-deftest locus-auth-l-absence-se-constate-sans-lever-d-erreur ()
  "Demander s'il y a un credential ne doit pas coûter une erreur : c'est la
question que pose un affichage de statut, et un statut ne plante pas."
  (locus-auth-test--without-credential
    (should-not (locus-auth-available-p)))
  (locus-auth-test--with-credential
    (should (locus-auth-available-p))))

;; ------------------------------------------------------------------------
;; Le secret ne sort pas d'auth-source
;; ------------------------------------------------------------------------

(ert-deftest locus-auth-le-credential-est-prete-jamais-rendu ()
  "La forme du module : `call-with-credential' **prête** le secret le temps
d'un appel.  Aucune fonction publique ne le renvoie — s'il en existait une,
les quatre interdits de §6.2 dépendraient de la discipline de chaque appelant."
  (locus-auth-test--with-credential
    (should (equal (locus-auth-call-with-credential (lambda (_) 'vu)) 'vu))
    (should-not (fboundp 'locus-auth-credential))
    (should-not (boundp 'locus-auth--cached-credential))))

(defun locus-auth-test--test-symbols ()
  "Les symboles définis par les fichiers de test.

Ce sont les seuls à exclure : les fixtures portent le secret par construction."
  (let ((tests (expand-file-name
                "test/" (file-name-as-directory
                         (expand-file-name (file-name-directory (locate-library "locus"))))))
        symbols)
    (dolist (entry load-history symbols)
      (let ((origin (and (stringp (car entry)) (expand-file-name (car entry)))))
        (when (and origin (string-prefix-p tests origin))
          (dolist (definition (cdr entry))
            (cond ((symbolp definition) (push definition symbols))
                  ((and (consp definition) (eq (car definition) 'defvar))
                   (push (cdr definition) symbols)))))))))

(defun locus-auth-test--package-symbols ()
  "Les symboles susceptibles de retenir un secret, fixtures de test exclues.

Deux critères réunis, et il faut les deux.  L'emplacement, via `load-history',
attrape ce que le paquet **déclare** quel que soit son nom.  Le préfixe attrape
ce qu'un `setq' crée sans `defvar' — une variable ainsi fabriquée n'entre pas
dans `load-history', et c'est justement la façon négligée d'ajouter un cache.
Le premier critère seul laissait passer la mutation qui met le secret en
cache ; le second seul écarterait tout le paquet le jour où un symbole ne porte
pas le préfixe."
  (let ((excluded (locus-auth-test--test-symbols))
        symbols)
    (mapatoms (lambda (symbol)
                (when (and (string-prefix-p "locus-" (symbol-name symbol))
                           (not (memq symbol excluded)))
                  (push symbol symbols))))
    symbols))

(ert-deftest locus-auth-aucune-variable-ne-retient-le-secret ()
  "Après un appel complet, aucune variable du paquet ne porte le secret.

Le test balaie **toutes** les variables du paquet plutôt que d'en vérifier
deux ou trois : une liste écrite à la main n'attrape que ce qu'on avait déjà
en tête, et le cache qu'on ajoutera plus tard n'y sera pas."
  (locus-auth-test--with-credential
    (locus-auth-authorization '((:method . "GET") (:path . "/health")))
    (let ((offenders (seq-filter
                      (lambda (symbol)
                        (and (boundp symbol)
                             (locus-auth-test--holds-secret-p (symbol-value symbol))))
                      (locus-auth-test--package-symbols))))
      (should (null offenders)))))

(defun locus-auth-test--holds-secret-p (value)
  "Renvoyer non-nil quand VALUE donne accès au secret.

« Donne accès », pas « contient » : `auth-source' rend le credential sous forme
d'une **fonction** d'accès, et garder cette fonction est garder le secret — il
suffit de l'appeler.  Un test qui ne chercherait que des chaînes laisserait
passer le cache le plus naturel à écrire, puisque c'est la valeur que
`auth-source' pose sous la main."
  (cond
   ((stringp value) (string-match-p (regexp-quote locus-auth-test--secret) value))
   ((functionp value)
    (let ((yielded (ignore-errors (funcall value))))
      (and (stringp yielded)
           (string-match-p (regexp-quote locus-auth-test--secret) yielded))))
   ((consp value) (or (locus-auth-test--holds-secret-p (car value))
                      (locus-auth-test--holds-secret-p (cdr value))))
   (t nil)))

(ert-deftest locus-auth-le-secret-ne-passe-pas-par-le-kill-ring ()
  "§6.2 : « pas de copie dans kill-ring »."
  (let ((kill-ring nil))
    (locus-auth-test--with-credential
      (locus-auth-authorization '((:method . "GET"))))
    (should (null kill-ring))))

(ert-deftest locus-auth-aucune-option-sauvegardable-ne-porte-le-secret ()
  "§6.2 : « aucun token dans `custom-file' ».

Ce que `custom-file' peut écrire est ce qui est `defcustom'.  Le test vérifie
donc qu'aucune option du paquet ne détient le secret — l'hôte et le login sont
des options, le credential n'en est pas une, et ne doit jamais le devenir."
  (locus-auth-test--with-credential
    (locus-auth-authorization '((:method . "GET")))
    (let ((offenders (seq-filter
                      (lambda (symbol)
                        (and (custom-variable-p symbol)
                             (locus-auth-test--holds-secret-p (symbol-value symbol))))
                      (locus-auth-test--package-symbols))))
      (should (null offenders)))))

;; ------------------------------------------------------------------------
;; Ce qui part avec le secret, et ce qui se journalise
;; ------------------------------------------------------------------------

(ert-deftest locus-auth-la-requete-autorisee-porte-l-en-tete ()
  "Le secret doit bien arriver quelque part, sinon le module ne sert à rien."
  (locus-auth-test--with-credential
    (let* ((request (locus-auth-authorization '((:method . "GET") (:path . "/v1/health"))))
           (header (alist-get "Authorization" (alist-get :headers request) nil nil #'equal)))
      (should (equal header (concat "Bearer " locus-auth-test--secret)))
      (should (equal (alist-get :path request) "/v1/health")))))

(ert-deftest locus-auth-la-requete-expurgee-ne-porte-plus-rien ()
  "§6.2 : « aucun token dans les messages de debug ».  C'est cette forme-ci qui
va dans un journal, et elle garde le **nom** de l'en-tête : savoir qu'une
autorisation était présente fait partie du diagnostic."
  (locus-auth-test--with-credential
    (let* ((request (locus-auth-authorization '((:method . "GET"))))
           (safe (locus-auth-redact request))
           (header (alist-get "Authorization" (alist-get :headers safe) nil nil #'equal)))
      (should (equal header "<expurgé>"))
      (should-not (locus-auth-test--holds-secret-p safe)))))

(ert-deftest locus-auth-l-expurgation-couvre-les-cookies-aussi ()
  "Un jeton n'est pas toujours dans `Authorization'.  Expurger ce seul en-tête
laisserait passer la forme la plus courante après lui — le cookie de session."
  (let* ((request '((:headers . (("Cookie" . "session=abc")
                                 ("X-Api-Key" . "k")
                                 ("Accept" . "application/json")))))
         (safe (locus-auth-redact request))
         (headers (alist-get :headers safe)))
    (should (equal (alist-get "Cookie" headers nil nil #'equal) "<expurgé>"))
    (should (equal (alist-get "X-Api-Key" headers nil nil #'equal) "<expurgé>"))
    (should (equal (alist-get "Accept" headers nil nil #'equal) "application/json")))
  ;; La casse ne protège pas : un en-tête écrit autrement porte le même secret.
  (let* ((safe (locus-auth-redact '((:headers . (("AUTHORIZATION" . "Bearer x"))))))
         (headers (alist-get :headers safe)))
    (should (equal (alist-get "AUTHORIZATION" headers nil nil #'equal) "<expurgé>"))))

;; ------------------------------------------------------------------------
;; Le changement d'origine
;; ------------------------------------------------------------------------

(ert-deftest locus-auth-un-changement-d-origine-est-refuse ()
  "§6.2, dernier point.  Le refus est dans le code et non dans une invite :
envoyer un credential à un hôte que l'utilisateur n'a pas choisi est la faute
que la confirmation existe pour empêcher, et une invite qu'un appelant oublie
d'afficher ne l'empêche pas."
  (should-error (locus-auth-check-endpoint "https://locus.example" "https://ailleurs.example")
                :type 'locus-auth-endpoint-changed))

(ert-deftest locus-auth-le-port-et-le-schema-font-partie-de-l-origine ()
  "Changer de port ou passer de https à http change l'interlocuteur autant que
changer de domaine — et c'est le cas qu'une comparaison de noms d'hôte rate."
  (should-error (locus-auth-check-endpoint "https://locus.example" "http://locus.example")
                :type 'locus-auth-endpoint-changed)
  (should-error (locus-auth-check-endpoint "http://127.0.0.1:7420" "http://127.0.0.1:9999")
                :type 'locus-auth-endpoint-changed))

(ert-deftest locus-auth-le-port-implicite-vaut-le-port-ecrit ()
  "`https://locus.example' et `https://locus.example:443' sont le même
interlocuteur : les distinguer produirait une confirmation devant un
changement qui n'en est pas un, et une confirmation qui se déclenche pour rien
finit par être cliquée sans être lue."
  (should (locus-auth-same-origin-p "https://locus.example" "https://locus.example:443"))
  (should (locus-auth-same-origin-p "http://locus.example" "http://locus.example:80")))

(ert-deftest locus-auth-le-premier-endpoint-ne-declenche-rien ()
  "Il n'y a pas de changement quand il n'y avait rien avant."
  (should (equal (locus-auth-check-endpoint nil "https://locus.example")
                 "https://locus.example"))
  (should (equal (locus-auth-check-endpoint "https://locus.example" "https://locus.example/v1")
                 "https://locus.example/v1")))

;; ------------------------------------------------------------------------
;; L'identité affichable
;; ------------------------------------------------------------------------

(ert-deftest locus-auth-le-principal-s-affiche-le-secret-non ()
  "§6.3 demande d'afficher le principal.  Le confondre avec le secret rendrait
l'identité invisible ; les confondre dans l'autre sens la rendrait dangereuse."
  (locus-auth-test--with-credential
    (should (equal (locus-auth-principal) "marcel"))
    (should-not (locus-auth-test--holds-secret-p (locus-auth-principal)))))

(provide 'locus-auth-test)

;;; locus-auth-test.el ends here
