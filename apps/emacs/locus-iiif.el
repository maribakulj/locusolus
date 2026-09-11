;;; locus-iiif.el --- Ouvrir dans xiiif ce qu'une mission a trouvé  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Un corpus produit par une mission s'ouvre dans le viewer, depuis le cockpit.**
;;
;; `xiiif' sait montrer un manifeste IIIF ; l'orchestrateur sait en produire une
;; liste.  Rien ne les reliait : il fallait lire un JSON dans un terminal,
;; recopier une URL, et la coller dans une commande.  Trois gestes pour ce qui
;; en demande un.
;;
;; # Ce module ne réimplémente rien
;;
;; `xiiif' est un logiciel à part (ADR 0007), et ce fichier ne touche ni à son
;; rendu, ni à son cache, ni à son transport.  Il fait une seule chose : lire un
;; corpus produit par une mission et appeler `xiiif-open-manifest'.  Si xiiif
;; n'est pas installé, il le dit et ne charge rien — un cockpit ne doit pas
;; dépendre d'un viewer pour s'ouvrir.
;;
;; # Le corpus est une donnée, pas du code
;;
;; Il arrive d'un worker, donc de l'extérieur.  Rien de ce qu'il contient n'est
;; évalué, et les URLs sont vérifiées avant d'être suivies : `CLAUDE.md' interdit
;; d'évaluer un contenu Locus comme de l'Elisp, et une liste d'URLs venue d'un
;; agent est exactement le genre d'entrée qui mérite cette garde.

;;; Code:

(require 'cl-lib)
(require 'locus)

(defgroup locus-iiif nil
  "Le pont entre un corpus de mission et le viewer IIIF."
  :group 'locus
  :prefix "locus-iiif-")

(defcustom locus-iiif-corpus-file nil
  "Le fichier JSON de corpus à ouvrir, ou nil pour le demander.

Un chemin **local** : le corpus est produit sur le worker, et c'est au
déploiement de dire comment il arrive ici — copie, montage, artefact.  Le
deviner ferait supposer une topologie que ce module n'a pas à connaître."
  :type '(choice (const :tag "demander" nil) file)
  :group 'locus-iiif)

(defconst locus-iiif-buffer "*Locus IIIF*"
  "Où le corpus se liste.")

(defun locus-iiif--url-sure-p (url)
  "Vrai quand URL est une adresse que l'on accepte de suivre.

`https' seulement, et un hôte non vide.  Un corpus vient d'un agent : il n'y a
aucune raison d'y trouver un `file://' ou un `javascript:', et toutes les
raisons de refuser de le suivre si on en trouve un."
  (and (stringp url)
       (string-match-p "\\`https://[^/ \t\n]+/" url)))

(defun locus-iiif-lire-corpus (fichier)
  "Lire FICHIER et rendre ses entrées, en écartant celles qu'on ne suivra pas.

Rend une liste d'alists.  Une entrée sans URL sûre est **écartée et comptée** :
la taire ferait croire à un corpus plus petit qu'il n'est, et l'accepter ferait
suivre une adresse qu'on a décidé de ne pas suivre."
  (unless (file-readable-p fichier)
    (user-error "corpus illisible : %s" fichier))
  (let* ((brut (with-temp-buffer
                 (insert-file-contents fichier)
                 (json-parse-buffer :object-type 'alist :array-type 'list
                                    :null-object nil :false-object nil)))
         (entrees (if (listp brut) brut (list brut)))
         (gardees nil)
         (ecartees 0))
    (dolist (e entrees)
      (if (and (consp e) (locus-iiif--url-sure-p (alist-get 'url e)))
          (push e gardees)
        (cl-incf ecartees)))
    (cons (nreverse gardees) ecartees)))

(defvar-local locus-iiif--entrees nil
  "Les entrées affichées, dans l'ordre des lignes.")

(defvar locus-iiif-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "RET") #'locus-iiif-ouvrir)
    (define-key map (kbd "g") #'locus-iiif)
    (define-key map (kbd "q") #'quit-window)
    map)
  "Le clavier de la liste de corpus.")

(define-derived-mode locus-iiif-mode special-mode "Locus-IIIF"
  "Liste des manifestes d'un corpus de mission."
  (setq-local truncate-lines t))

(defun locus-iiif-ouvrir ()
  "Ouvrir dans xiiif le manifeste de la ligne courante."
  (interactive)
  (let ((entree (nth (1- (line-number-at-pos)) locus-iiif--entrees)))
    (unless entree
      (user-error "aucun manifeste sur cette ligne"))
    (unless (fboundp 'xiiif-open-manifest)
      (user-error "xiiif n'est pas installé : M-x marcel-xiiif-install"))
    (let ((url (alist-get 'url entree)))
      (message "xiiif : %s" url)
      (funcall (intern "xiiif-open-manifest") url))))

;;;###autoload
(defun locus-iiif (&optional fichier)
  "Lister le corpus FICHIER, et l'ouvrir d'un `RET' dans xiiif."
  (interactive)
  (let* ((fichier (or fichier locus-iiif-corpus-file
                      (read-file-name "Corpus JSON : " nil nil t)))
         (lu (locus-iiif-lire-corpus fichier))
         (entrees (car lu))
         (ecartees (cdr lu))
         (buffer (get-buffer-create locus-iiif-buffer)))
    (with-current-buffer buffer
      (let ((inhibit-read-only t))
        (erase-buffer)
        (locus-iiif-mode)
        (setq locus-iiif--entrees entrees)
        (dolist (e entrees)
          (insert (format "%5s  %s\n"
                          (or (alist-get 'canvases e) "?")
                          (or (alist-get 'label e) "(sans titre)"))))
        (goto-char (point-min))
        (setq header-line-format
              (format " %d manifeste(s)%s   [RET] ouvrir dans xiiif  [g] relire  [q] quitter"
                      (length entrees)
                      (if (> ecartees 0)
                          (format " · %d écartée(s), URL non suivie" ecartees)
                        "")))))
    (pop-to-buffer buffer)
    buffer))

(provide 'locus-iiif)

;;; locus-iiif.el ends here
