;;; locus-dashboard.el --- Le tableau de bord, reconstruit depuis le cache  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; `SPEC.md' §9.1 et §22.2.
;;
;; # Le rendu ne parle à personne
;;
;; C'est la propriété de sortie du sprint, et elle est plus forte qu'un confort
;; hors ligne : un rendu qui interroge le serveur rend le tableau de bord aussi
;; disponible que le réseau, alors qu'il sert précisément à savoir ce qui se
;; passe quand quelque chose ne va pas.  §14.1 le dit d'ailleurs de l'autre
;; côté — « n'effectue pas une query complète à chaque événement ».
;;
;; Le rendu est donc une **fonction du cache**.  Ce qui manque au cache manque
;; à l'écran, et s'y voit : §22.2 exige que toute donnée offline affiche sa
;; dernière synchronisation, son curseur et son état `stale'.  Une ligne muette
;; sur sa fraîcheur est pire qu'une ligne absente, parce qu'elle se lit comme
;; une ligne à jour.

;;; Code:

(require 'cl-lib)
(require 'tabulated-list)
(require 'locus-cache)

(defconst locus-dashboard-columns
  [("Program" 20 t)
   ("Status" 10 t)
   ("Branches" 9 t)
   ("Tasks" 6 t)
   ("Agents" 7 t)
   ("Reviews" 8 t)
   ("Budget" 10 t)
   ("Last event" 12 t)
   ("Risk" 6 t)]
  "Les colonnes minimales de §9.1.")

(defconst locus-dashboard-buffer-name "*Locus Solus Dashboard*"
  "Le nom exact que §9.1 donne au tampon.")

(defun locus-dashboard-rows (keys)
  "Les lignes du tableau, construites depuis le cache pour KEYS.

Une clé absente du cache ne produit pas de ligne : inventer une ligne vide
laisserait croire à un programme sans activité, ce qui est une information, et
une information fausse."
  (delq nil
        (mapcar (lambda (key)
                  (let ((entry (locus-cache-get key)))
                    (when entry
                      (list key (locus-dashboard--columns entry)))))
                keys)))

(defun locus-dashboard--columns (entry)
  "Le vecteur de colonnes pour ENTRY."
  (let ((program (locus-cache-entry-value entry)))
    (vector (or (alist-get :name program) "?")
            (locus-dashboard--status entry program)
            (number-to-string (or (alist-get :branches program) 0))
            (number-to-string (or (alist-get :tasks program) 0))
            (number-to-string (or (alist-get :agents program) 0))
            (number-to-string (or (alist-get :reviews program) 0))
            (or (alist-get :budget program) "—")
            (or (alist-get :last-event program) "—")
            (or (alist-get :risk program) "—"))))

(defun locus-dashboard--status (entry program)
  "Le statut affiché, fraîcheur comprise — §22.2.

La péremption prend le pas sur le statut rapporté : afficher `active' sur une
donnée vieille d'un jour serait exact au moment où elle a été lue et faux à
l'écran, ce qui est la seule des deux choses que l'utilisateur voit."
  (if (locus-cache-stale-p entry)
      "stale"
    (or (alist-get :status program) "?")))

(defun locus-dashboard-header (keys)
  "La ligne d'en-tête : dernière synchronisation, curseur, état — §22.2."
  (let* ((entries (delq nil (mapcar #'locus-cache-get keys)))
         (oldest (car (sort (mapcar #'locus-cache-age entries) #'>)))
         (cursor (car (delq nil (mapcar #'locus-cache-entry-cursor entries))))
         (stale (cl-some #'locus-cache-stale-p entries)))
    (format "Locus Solus — synchronisé il y a %s, curseur %s%s"
            (if oldest (format "%ds" (round oldest)) "jamais")
            (or cursor "—")
            (if stale " — STALE" ""))))

(define-derived-mode locus-dashboard-mode tabulated-list-mode "Locus"
  "Mode du tableau de bord Locus Solus."
  (setq tabulated-list-format locus-dashboard-columns)
  (setq tabulated-list-padding 1)
  (tabulated-list-init-header))

;;;###autoload
(defun locus-dashboard-render (keys)
  "Construire le tampon du tableau de bord pour KEYS, et le renvoyer.

# Aucun accès réseau

La fonction ne lit que le cache.  C'est vérifié plutôt que promis : le test de
sortie empoisonne `url-retrieve', `make-network-process' et
`open-network-stream' avant d'appeler, de sorte qu'un rendu qui parlerait à
quiconque échouerait au lieu de réussir plus lentement."
  (let ((buffer (get-buffer-create locus-dashboard-buffer-name)))
    (with-current-buffer buffer
      (locus-dashboard-mode)
      (setq tabulated-list-entries (locus-dashboard-rows keys))
      (setq header-line-format (locus-dashboard-header keys))
      (tabulated-list-print t))
    buffer))

(provide 'locus-dashboard)

;;; locus-dashboard.el ends here
