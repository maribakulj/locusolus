;;; locus-auth.el --- Identité et credentials, via auth-source  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; `SPEC.md' §6.  Le credential vit dans `auth-source' et **n'en sort pas**.
;;
;; # La forme du module vient de sa liste d'interdits
;;
;; §6.2 énumère ce qui ne doit pas arriver : aucun token dans Git, dans
;; `custom-file', dans un message de debug, dans le kill-ring.  Ces quatre
;; interdits ont la même cause — un secret rangé dans une variable finit par
;; être sauvegardé, affiché ou copié, parce que c'est ce qu'on fait des
;; variables.  Le module ne les traite donc pas un par un : il ne garde jamais
;; le secret.
;;
;; D'où `locus-auth-call-with-credential', qui **prête** le credential le temps
;; d'un appel au lieu de le rendre.  Un `locus-auth-credential' qui renverrait
;; la chaîne serait plus commode et rendrait les quatre interdits dépendants de
;; la discipline de chaque appelant.  Ici il n'y a pas de discipline à tenir :
;; la valeur n'existe pas assez longtemps pour être rangée quelque part.
;;
;; # Une identité absente n'est pas une panne
;;
;; C'est le cas le plus fréquent — première installation, machine neuve,
;; entrée expirée — et le pire endroit pour un backtrace.  `locus-auth-missing'
;; dit quelle ligne écrire et dans quel fichier.  Une erreur actionnable est
;; celle dont le message contient le geste suivant.

;;; Code:

(require 'auth-source)
(require 'locus)
(require 'url-parse)

(defcustom locus-auth-host "locus.local"
  "Hôte sous lequel `auth-source' range le credential — `SPEC.md' §6.1.

Configurable parce que l'hôte du netrc n'est pas forcément celui de
l'endpoint : une même machine peut parler à plusieurs déploiements, et
l'inverse."
  :type 'string
  :group 'locus)

(defcustom locus-auth-user nil
  "Login sous lequel chercher le credential, ou nil pour ne pas filtrer.

nil est un défaut sûr : il laisse `auth-source' choisir l'entrée de l'hôte
plutôt que d'imposer un nom d'utilisateur qui serait, lui, une donnée
personnelle dans un fichier de configuration partagé."
  :type '(choice (const :tag "n'importe lequel" nil) string)
  :group 'locus)

(define-error 'locus-auth-missing
              "Aucun credential Locus Solus trouvé dans auth-source"
              'locus-error)

(define-error 'locus-auth-endpoint-changed
              "L'endpoint Locus Solus a changé d'origine"
              'locus-error)

(defun locus-auth--entry ()
  "L'entrée `auth-source' du déploiement courant, ou nil."
  (car (apply #'auth-source-search
              :host locus-auth-host
              :max 1
              :require '(:secret)
              (when locus-auth-user (list :user locus-auth-user)))))

(defun locus-auth-principal ()
  "Le login associé au credential, ou nil s'il n'y en a pas.

Le principal n'est pas un secret : §6.3 demande de l'afficher.  C'est le mot
de passe qui ne sort pas, et les confondre rendrait l'identité invisible."
  (plist-get (locus-auth--entry) :user))

(defun locus-auth-available-p ()
  "Renvoyer non-nil quand un credential existe.

Ne le lit pas.  Savoir s'il y en a un et le lire sont deux questions
différentes, et la première n'a pas besoin de la seconde."
  (and (locus-auth--entry) t))

(defun locus-auth-call-with-credential (function)
  "Appeler FUNCTION avec le credential en argument, et l'oublier ensuite.

Le secret n'est ni renvoyé, ni mis en cache, ni journalisé : il n'existe que
pendant l'appel.  C'est ce qui rend les quatre interdits de §6.2 vrais par
construction plutôt que par la vigilance de chaque appelant.

FUNCTION reçoit une chaîne.  Sa valeur de retour est celle de cet appel — à
charge pour elle de ne pas rendre le secret qu'on vient de lui prêter.

# Errors

`locus-auth-missing' quand aucune entrée ne correspond, avec la ligne exacte
à écrire : une erreur qui ne dit pas le geste suivant oblige à lire le code."
  (let ((entry (locus-auth--entry)))
    (unless entry
      (signal 'locus-auth-missing
              (list (format "ajoutez à ~/.authinfo.gpg : machine %s login <vous> password <credential>"
                            locus-auth-host))))
    (let ((secret (plist-get entry :secret)))
      (funcall function (if (functionp secret) (funcall secret) secret)))))

(defun locus-auth-authorization (request)
  "Appliquer l'autorisation à REQUEST et renvoyer la requête complétée.

REQUEST est une alist ; l'en-tête `Authorization' est ajouté sous `:headers'.
La requête ainsi complétée porte le secret : elle est destinée à partir
immédiatement, et ne doit pas être conservée.  C'est la raison pour laquelle
cette fonction ne met rien en cache — la version sans secret est
`locus-auth-redact', et c'est elle qui va dans les journaux."
  (locus-auth-call-with-credential
   (lambda (credential)
     (cons (cons :headers
                 (cons (cons "Authorization" (concat "Bearer " credential))
                       (alist-get :headers request)))
           (assq-delete-all :headers (copy-alist request))))))

(defun locus-auth-redact (request)
  "REQUEST sans ce qui ne doit pas être journalisé — §6.2.

Expurge par **nom d'en-tête**, pas par valeur : chercher le secret dans le
texte supposerait qu'on l'ait sous la main pour le comparer, ce qui est
exactement ce que le module refuse de faire."
  (let ((headers (alist-get :headers request)))
    (cons (cons :headers
                (mapcar (lambda (header)
                          (if (locus-auth--sensitive-p (car header))
                              (cons (car header) "<expurgé>")
                            header))
                        headers))
          (assq-delete-all :headers (copy-alist request)))))

(defconst locus-auth--sensitive-headers
  '("authorization" "cookie" "set-cookie" "proxy-authorization" "x-api-key")
  "Les en-têtes dont la valeur ne se journalise pas.")

(defun locus-auth--sensitive-p (name)
  "Renvoyer non-nil quand l'en-tête NAME porte un secret."
  (member (downcase name) locus-auth--sensitive-headers))

(defun locus-auth-same-origin-p (left right)
  "Renvoyer non-nil quand LEFT et RIGHT ont la même origine.

Schéma, hôte et port — la définition ordinaire, et celle qui compte : changer
de port ou passer de https à http change l'interlocuteur autant que changer de
domaine."
  (let ((a (url-generic-parse-url left))
        (b (url-generic-parse-url right)))
    (and (equal (url-type a) (url-type b))
         (equal (url-host a) (url-host b))
         (equal (locus-auth--port a) (locus-auth--port b)))))

(defun locus-auth--port (url)
  "Le port explicite de URL, ou celui que son schéma implique."
  (or (url-portspec url)
      (pcase (url-type url) ("https" 443) ("http" 80) (_ nil))))

(defun locus-auth-check-endpoint (previous current)
  "Vérifier le passage de PREVIOUS à CURRENT — §6.2, dernier point.

# Errors

`locus-auth-endpoint-changed' quand l'origine change.  Le refus est ici et non
dans une invite : envoyer un credential à un hôte que l'utilisateur n'a pas
choisi est la faute que la confirmation existe pour empêcher, et une invite
qu'un appelant oublie d'afficher ne l'empêche pas.  L'appelant rattrape
l'erreur pour demander, il ne décide pas de la poser."
  (when (and previous (not (locus-auth-same-origin-p previous current)))
    (signal 'locus-auth-endpoint-changed (list previous current)))
  current)

(provide 'locus-auth)

;;; locus-auth.el ends here
