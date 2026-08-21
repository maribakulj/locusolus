;;; locus-author-test.el --- Test de sortie de W8.j  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Deux propriétés distinctes, deux tests** — c'est ce que `docs/10' exige,
;; et la raison mérite d'être écrite ici parce qu'elle est facile à perdre :
;;
;; - `canonical(parse(t))' est **invariante** par ajout de commentaire et par
;;   réordonnancement ;
;; - `parse(write(p))' rend la **valeur** `p'.
;;
;; Elles ne se déduisent pas l'une de l'autre.  Et la formule qu'on écrirait
;; spontanément — `parse(write(t)) = t' sur le **texte** — est fausse : `write'
;; ne restitue ni les commentaires ni l'ordre d'origine, et c'est exactement ce
;; qui rend les deux formes distinctes.

;;; Code:

(require 'ert)
(require 'locus-author)

(defconst locus-author-test--source
  (expand-file-name "../locus-author.el"
                    (file-name-directory
                     (or load-file-name buffer-file-name default-directory)))
  "Le chemin du module, capturé **au chargement**.

`load-file-name' n'est lié que pendant le chargement : le lire depuis
le corps d'un test rend nil, et `expand-file-name' échoue alors sur un
type au lieu de dire ce qui manque.")

(defconst locus-author-test--texte
  ";; une politique de garde\nkind: policy\nname: garde-budget\nverb: deny  ; le verbe\nscope: programme\n"
  "Une forme d'écriture ordinaire : commentaire d'en-tête, commentaire en ligne.")

;; ---------------------------------------------------------------------------
;; 1 — la forme canonique ne bouge pas quand l'écriture bouge
;; ---------------------------------------------------------------------------

(ert-deftest locus-author-test-canonique-invariante-par-commentaire ()
  "Un commentaire ajouté laisse le condensat inchangé.

Si ce n'était pas vrai, le même document approuvé deux fois n'aurait
pas la même signature, et une relecture qui ajoute une note casserait
une approbation."
  (let* ((nu "kind: policy\nname: garde-budget\nverb: deny\nscope: programme\n")
         (commente (concat ";; en-tête\n" nu ";; et une note de fin\n")))
    (should (equal (locus-author-canonical (locus-author-parse nu))
                   (locus-author-canonical (locus-author-parse commente))))))

(ert-deftest locus-author-test-canonique-invariante-par-reordonnancement ()
  "L'ordre de frappe n'entre pas dans ce qui est signé."
  (let ((un "kind: policy\nname: g\nverb: deny\nscope: programme\n")
        (autre "scope: programme\nverb: deny\nname: g\nkind: policy\n"))
    (should (equal (locus-author-canonical (locus-author-parse un))
                   (locus-author-canonical (locus-author-parse autre))))))

(ert-deftest locus-author-test-la-forme-d-ecriture-n-est-jamais-la-canonique ()
  "Les deux formes sont deux objets, et le test le montre au lieu de le dire."
  (let* ((document (locus-author-parse locus-author-test--texte))
         (ecriture (locus-author-write document))
         (canonique (locus-author-canonical document)))
    (should-not (equal ecriture canonique))
    ;; L'écriture porte un commentaire ; la canonique n'en porte aucun.
    (should (string-match-p "^;;" ecriture))
    (should-not (string-match-p ";;" canonique))
    ;; Et la canonique porte son en-tête de version, que l'écriture n'a pas.
    (should (string-prefix-p "author/1\n" canonique))))

;; ---------------------------------------------------------------------------
;; 2 — l'écriture est fidèle à la valeur
;; ---------------------------------------------------------------------------

(ert-deftest locus-author-test-relire-ce-qu-on-ecrit-rend-la-valeur ()
  "`parse(write(p))' rend `p' — sur la **valeur**, jamais sur le texte."
  (let* ((document (locus-author-parse locus-author-test--texte))
         (relu (locus-author-parse (locus-author-write document))))
    (should (equal (locus-author-document-kind relu)
                   (locus-author-document-kind document)))
    (should (equal (locus-author-document-name relu)
                   (locus-author-document-name document)))
    (should (equal (locus-author-document-fields relu)
                   (locus-author-document-fields document)))
    ;; Et le condensat suit, ce qui est la conséquence utile.
    (should (equal (locus-author-canonical relu)
                   (locus-author-canonical document)))))

(ert-deftest locus-author-test-le-texte-ne-se-restitue-pas ()
  "La formule naïve est fausse, et le test la nomme pour qu'on ne l'écrive pas.

`parse(write(t))' rend bien la valeur, mais `write(parse(t))' ne rend
pas `t' : les commentaires et l'ordre d'origine ne sont pas dans la
valeur.  Confondre les deux ferait écrire un test qui échoue pour une
raison saine, et on l'affaiblirait au lieu de le comprendre."
  (should-not (equal (locus-author-write (locus-author-parse locus-author-test--texte))
                     locus-author-test--texte)))

;; ---------------------------------------------------------------------------
;; 3 — rien ne s'applique sur place
;; ---------------------------------------------------------------------------

(ert-deftest locus-author-test-la-commande-ne-modifie-rien ()
  "La commande d'auteur **rend** un document ; elle n'écrit nulle part."
  (with-temp-buffer
    (insert locus-author-test--texte)
    ;; Repartir de « non modifié » : c'est l'`insert' du test qui vient de marquer le
    ;; tampon, et l'assertion d'origine testait donc son propre montage plutôt que la
    ;; commande. Sans cette remise à zéro, elle aurait échoué même sur une commande
    ;; parfaitement inerte — un test qui ne peut pas passer ne garde rien.
    (set-buffer-modified-p nil)
    (let* ((avant (buffer-string))
           (document (locus-author-proposal-from-buffer)))
      (should (locus-author-document-p document))
      (should (equal (buffer-string) avant))
      (should-not (buffer-modified-p)))))

(ert-deftest locus-author-test-aucun-chemin-n-applique-le-document ()
  "Tenu par l'absence : le module ne connaît ni transport ni écriture.

Les motifs visent des **appels**, pas des mots : la documentation du
module emploie « soumettre » pour dire précisément ce que ce module ne
fait pas, et une garde qui se déclenche sur sa propre justification est
une garde qu'on finit par assouplir."
  (let ((source (with-temp-buffer
                  (insert-file-contents locus-author-test--source)
                  (buffer-string))))
    (dolist (interdit '("(locus-command-submit" "(locus-http-" "(url-retrieve"
                        "(write-region" "(save-buffer"))
      (should-not (string-search interdit source)))))

;; ---------------------------------------------------------------------------
;; 4 — ce que la forme d'écriture refuse
;; ---------------------------------------------------------------------------

(ert-deftest locus-author-test-une-ligne-indechiffrable-est-refusee ()
  "Deviner reviendrait à écrire à la place de l'auteur."
  (should-error (locus-author-parse "kind: policy\nname: g\nune ligne sans deux-points\n")
                :type 'locus-author-invalid)
  (should-error (locus-author-parse "kind: policy\nname: g\nverb:\n")
                :type 'locus-author-invalid)
  (should-error (locus-author-parse "kind: inconnue\nname: g\n")
                :type 'locus-author-invalid)
  (should-error (locus-author-parse "name: g\n") :type 'locus-author-invalid)
  (should-error (locus-author-parse "kind: policy\n") :type 'locus-author-invalid))

(ert-deftest locus-author-test-un-champ-en-double-est-refuse ()
  "Garder le premier ou le dernier serait un choix que personne n'a écrit."
  (should-error (locus-author-parse "kind: policy\nname: g\nverb: deny\nverb: allow\n")
                :type 'locus-author-invalid))

(ert-deftest locus-author-test-un-point-virgule-echappe-est-une-donnee ()
  "Sans l'échappée, un champ ne pourrait pas contenir de point-virgule.

Et personne ne le devinerait avant d'y perdre du texte — le genre de
perte qui ne se voit qu'une fois le document approuvé."
  (let ((document (locus-author-parse "kind: policy\nname: g\nnote: a\\;b  ; commentaire\n")))
    (should (equal (cdr (assoc "note" (locus-author-document-fields document))) "a;b"))))

(ert-deftest locus-author-test-une-valeur-forgeant-une-ligne-est-refusee ()
  "Même durcissement que les cinq autres formes canoniques du dépôt.

Une valeur portant une fin de ligne insérerait dans le document signé
un champ que personne n'a écrit."
  (let ((forge (locus-author--document
                :kind 'policy :name "g"
                :fields (list (cons "note" "a\nfield\tverb\tallow")))))
    (should-error (locus-author-canonical forge) :type 'locus-author-invalid)))

(provide 'locus-author-test)
;;; locus-author-test.el ends here
