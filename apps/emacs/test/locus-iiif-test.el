;;; locus-iiif-test.el --- Ce que le pont vers xiiif doit tenir  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Un corpus produit par un agent s'ouvre, et ce qu'on n'accepte pas de suivre
;; ne se suit pas.**
;;
;; Le corpus arrive d'un worker, donc de l'extérieur.  Ces tests portent surtout
;; sur ce qui est **écarté** : une liste d'URLs venue d'un agent est exactement
;; le genre d'entrée dont on doit pouvoir dire ce qu'on en fait.

;;; Code:

(require 'ert)
(require 'locus-iiif)

(defun locus-iiif-test--fichier (contenu)
  "Écrire CONTENU dans un fichier temporaire et rendre son chemin."
  (let ((f (make-temp-file "locus-iiif-test" nil ".json")))
    (with-temp-file f (insert contenu))
    f))

(ert-deftest locus-iiif-un-corpus-se-lit-et-garde-son-ordre ()
  "**Le test de sortie.**  Le corpus d'une mission devient une liste ouvrable."
  (let* ((f (locus-iiif-test--fichier
             "[{\"url\":\"https://mdc.csuc.cat/iiif/2/a:1/manifest.json\",\"label\":\"Ars brevis\",\"canvases\":104},
               {\"url\":\"https://mdc.csuc.cat/iiif/2/a:2/manifest.json\",\"label\":\"Codicillus\",\"canvases\":217}]"))
         (lu (locus-iiif-lire-corpus f)))
    (should (= (length (car lu)) 2))
    (should (= (cdr lu) 0))
    (should (equal (alist-get 'label (car (car lu))) "Ars brevis"))
    (delete-file f)))

(ert-deftest locus-iiif-une-url-qu-on-ne-suit-pas-est-ecartee-et-comptee ()
  "Un corpus vient d'un agent : ce qu'on refuse de suivre se voit.

La taire ferait croire à un corpus plus petit qu'il n'est ; l'accepter ferait
suivre une adresse qu'on a décidé de ne pas suivre.  Les deux sont mauvais, et
le compte rendu est ce qui les évite tous les deux."
  (let* ((f (locus-iiif-test--fichier
             "[{\"url\":\"https://ok.example.org/iiif/manifest.json\",\"label\":\"bonne\"},
               {\"url\":\"file:///etc/passwd\",\"label\":\"fichier local\"},
               {\"url\":\"javascript:alert(1)\",\"label\":\"script\"},
               {\"url\":\"http://sans-tls.example.org/m.json\",\"label\":\"sans TLS\"},
               {\"label\":\"sans url du tout\"}]"))
         (lu (locus-iiif-lire-corpus f)))
    (should (= (length (car lu)) 1))
    (should (equal (alist-get 'label (car (car lu))) "bonne"))
    ;; Quatre écartées, et le nombre est rendu — pas seulement l'absence.
    (should (= (cdr lu) 4))
    (delete-file f)))

(ert-deftest locus-iiif-un-corpus-absent-refuse-avant-de-lire ()
  "Un fichier illisible est une erreur d'appel, nommée comme telle."
  (should-error (locus-iiif-lire-corpus "/n/existe/pas/corpus.json") :type 'user-error))

(ert-deftest locus-iiif-charger-n-exige-pas-xiiif ()
  "Le cockpit ne dépend pas du viewer pour s'ouvrir.

`xiiif' est un logiciel à part (ADR 0007).  Ce module le **référence** et ne le
charge pas : sur une machine sans xiiif, lister un corpus doit marcher, et
seule l'ouverture doit se plaindre."
  (should (featurep 'locus-iiif))
  (should-not (featurep 'xiiif)))

(provide 'locus-iiif-test)

;;; locus-iiif-test.el ends here
