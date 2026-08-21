;;; locus-author.el --- Rédiger une politique ou une proposition  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 Locus Solus
;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; `W8.j' — Emacs **auteur**.  §20 pour les politiques, §22.3 pour les
;; propositions.
;;
;; # Deux formes, et il ne faut jamais qu'elles soient la même
;;
;; La **forme d'écriture** est ce qu'un humain tape : commentaires, ordre
;; libre, espacement.  La **forme canonique** est ce sur quoi porte le
;; condensat, donc ce qu'une approbation signe (ADR 0020).
;;
;; Si les deux étaient le même objet, il faudrait choisir entre deux maux : ou
;; bien un commentaire ajouté changerait ce qu'une approbation signe — et le
;; même document approuvé deux fois n'aurait pas le même condensat —, ou bien
;; la canonicalisation devrait **dépouiller** la forme d'écriture, et le
;; dépouillement est précisément l'endroit où vivent les forgeries que `W17.h'
;; a découvertes.  Les séparer coûte un analyseur ; les confondre coûte la
;; signature.
;;
;; D'où deux propriétés, et deux tests, qu'il ne faut pas confondre :
;;
;; - `canonical(parse(t))' est **invariante** par ajout de commentaire et par
;;   réordonnancement — c'est ce qui rend le condensat stable ;
;; - `parse(write(p))' rend la **valeur** `p' — c'est ce qui rend l'écriture
;;   fidèle.
;;
;; Elles ne se déduisent pas l'une de l'autre, et écrire `parse(write(t)) = t'
;; sur le **texte** serait faux, puisque `write' ne restitue ni les
;; commentaires ni l'ordre d'origine.
;;
;; # Rien ne s'applique sur place
;;
;; Une commande d'auteur rend une proposition ; elle n'écrit nulle part.
;; `SPEC.md' §11 tient déjà cette règle pour les commandes mutantes, et il n'y
;; a aucune raison qu'un tampon d'édition soit le trou par lequel elle
;; s'échappe.

;;; Code:

(require 'cl-lib)
(require 'locus)

(define-error 'locus-author-invalid "Document d'auteur mal formé" 'locus-error)

(defconst locus-author-kinds '(policy proposal)
  "Ce qu'on peut rédiger ici — §20 et §22.3.

Deux, liste close.  Une troisième sorte n'entrera que lorsqu'un
consommateur exécutable la lira : un document qu'aucun destinataire
n'accepte est un fichier, pas une proposition.")

(cl-defstruct (locus-author-document
               (:constructor locus-author--document)
               (:copier nil))
  "Un document rédigé, sous sa forme de **valeur**.

Les champs sont ordonnés ici ; l'ordre du texte d'origine n'y survit
pas, et c'est voulu — c'est ce qui rend la forme canonique
indépendante de la frappe."
  (kind nil :read-only t)
  (name nil :read-only t)
  (fields nil :read-only t))

(defun locus-author--trim (text)
  "Retirer les blancs de bord de TEXT."
  (string-trim (or text "")))

(defun locus-author--strip-comment (line)
  "Retirer de LINE ce qui suit un point-virgule non protégé.

Le point-virgule est le commentaire d'Elisp, et c'est ce qu'un
rédacteur tapera sans y penser.  Un point-virgule **précédé d'une
barre oblique inverse** est une donnée : sans cette échappée, un
champ ne pourrait pas contenir de point-virgule, ce qu'aucun
rédacteur ne devinerait avant d'y perdre du texte."
  (let ((out (make-string 0 ?x))
        (index 0)
        (length (length line))
        (stop nil))
    (while (and (< index length) (not stop))
      (let ((char (aref line index)))
        (cond
         ((and (eq char ?\\) (< (1+ index) length) (eq (aref line (1+ index)) ?\;))
          (setq out (concat out ";"))
          (setq index (+ index 2)))
         ((eq char ?\;) (setq stop t))
         (t (setq out (concat out (string char)))
            (setq index (1+ index))))))
    out))

(defun locus-author-parse (text)
  "Lire TEXT sous sa forme d'écriture, et rendre un document.

La forme d'écriture est délibérément permissive : commentaires après
`;', lignes vides, ordre libre, espacement autour de `:'.  Ce qu'elle
n'admet pas est l'ambiguïté — une ligne sans `:' ne dit pas ce
qu'elle voudrait dire, et la deviner reviendrait à écrire à la place
de l'auteur.

Signale `locus-author-invalid' pour une sorte inconnue, un nom vide,
un champ sans valeur ou une ligne indéchiffrable."
  (let ((kind nil) (name nil) (fields nil))
    (dolist (raw (split-string (or text "") "\n"))
      (let ((line (locus-author--trim (locus-author--strip-comment raw))))
        (unless (string-empty-p line)
          (let ((cut (string-search ":" line)))
            (unless cut
              (signal 'locus-author-invalid (list "ligne sans « : »" line)))
            (let ((key (locus-author--trim (substring line 0 cut)))
                  (value (locus-author--trim (substring line (1+ cut)))))
              (when (string-empty-p key)
                (signal 'locus-author-invalid (list "clé vide" line)))
              (when (string-empty-p value)
                (signal 'locus-author-invalid (list "valeur vide" key)))
              (cond
               ((equal key "kind")
                (setq kind (intern value))
                (unless (memq kind locus-author-kinds)
                  (signal 'locus-author-invalid (list "sorte inconnue" value))))
               ((equal key "name") (setq name value))
               (t
                (when (assoc key fields)
                  (signal 'locus-author-invalid (list "champ en double" key)))
                (push (cons key value) fields))))))))
    (unless kind (signal 'locus-author-invalid (list "aucune sorte déclarée")))
    (unless name (signal 'locus-author-invalid (list "aucun nom déclaré")))
    (locus-author--document :kind kind :name name
                            :fields (sort fields (lambda (a b) (string< (car a) (car b)))))))

(defun locus-author-canonical (document)
  "Rendre la forme canonique de DOCUMENT — celle que le condensat couvre.

Un texte à lignes, **trié**, avec un en-tête de version.  Trié parce
qu'un multiensemble de champs n'a pas d'ordre : deux rédacteurs qui
tapent les mêmes champs dans un ordre différent décrivent la même
politique, et leurs approbations doivent porter sur le même document.

Les caractères de contrôle sont refusés dans les valeurs, comme dans
les cinq autres formes canoniques du dépôt : sans cela une valeur
forgerait une ligne, et un champ que personne n'a écrit entrerait
dans ce qui est signé."
  (let ((lines (list (format "author/1")
                     (format "kind\t%s" (locus-author-document-kind document))
                     (format "name\t%s" (locus-author-document-name document)))))
    (dolist (field (locus-author-document-fields document))
      (dolist (part (list (car field) (cdr field)))
        (when (string-match-p "[[:cntrl:]]" part)
          (signal 'locus-author-invalid (list "caractère de contrôle" part))))
      (setq lines (append lines (list (format "field\t%s\t%s" (car field) (cdr field))))))
    (concat (string-join lines "\n") "\n")))

(defun locus-author-write (document)
  "Rendre une forme d'**écriture** pour DOCUMENT.

Elle se relit — `locus-author-parse' de ce texte rend la même valeur —
mais elle n'est **pas** la forme canonique : elle porte un commentaire
d'en-tête, que la canonicalisation ignore.  C'est la démonstration la
plus courte que les deux formes sont deux choses."
  (let ((lines (list ";; Rédigé par Emacs — cette forme se relit, elle ne se signe pas."
                     (format "kind: %s" (locus-author-document-kind document))
                     (format "name: %s" (locus-author-document-name document)))))
    (dolist (field (locus-author-document-fields document))
      (setq lines (append lines (list (format "%s: %s" (car field) (cdr field))))))
    (concat (string-join lines "\n") "\n")))

(defun locus-author-proposal-from-buffer (&optional buffer)
  "Rendre le document rédigé dans BUFFER, sans rien appliquer.

C'est la commande d'auteur, et son contrat tient en un mot :
**rendre**.  Elle n'écrit pas, n'envoie pas, ne modifie pas le tampon.
Soumettre est le rôle de `locus-command-submit', qui exige une
révision attendue — et une commande d'auteur qui court-circuiterait
ce chemin appliquerait une mutation à un état que personne n'a vu."
  (interactive)
  (with-current-buffer (or buffer (current-buffer))
    (locus-author-parse (buffer-substring-no-properties (point-min) (point-max)))))

(provide 'locus-author)
;;; locus-author.el ends here
