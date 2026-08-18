;;; locus-artifact-test.el --- Test de sortie de W8.f  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Un artefact non promu se distingue d'un artefact promu à l'écran ; rien
;; n'est exécuté ; le paquet ne parle jamais à un runtime de containers.**
;;
;; La dernière propriété est celle de §20A, et c'est la seule du paquet qui se
;; vérifie sur le **texte** des sources : un client qui appellerait Podman
;; contournerait le control plane, et le contournement ne se voit pas dans le
;; comportement — il se voit dans ce qui est écrit.

;;; Code:

(require 'ert)
(require 'cl-lib)
(require 'locus-artifact)

;; ------------------------------------------------------------------------
;; Promu et non promu ne se ressemblent pas
;; ------------------------------------------------------------------------

(ert-deftest locus-artifact-promu-et-non-promu-ne-se-ressemblent-pas ()
  "Le test qui porte le sprint.

Un artefact `staged' affiché comme un `promoted' fait citer un résultat qui
n'a pas été validé.  L'invariant 4 — artifact-first et provenance-first — ne
tient pas si l'écran aplatit la différence."
  (let ((badges (mapcar #'locus-artifact-badge locus-artifact-states)))
    (ert-info ("six états, six badges distincts")
      (should (equal (length (delete-dups (copy-sequence badges))) 6)))
    (should-not (equal (locus-artifact-badge 'promoted)
                       (locus-artifact-badge 'verified)))
    (should-not (equal (locus-artifact-badge 'promoted)
                       (locus-artifact-badge 'quarantined)))))

(ert-deftest locus-artifact-un-etat-inconnu-ne-se-rend-pas-comme-du-connu ()
  "Aucun défaut rassurant : rendre l'inconnu comme du connu est la façon la
plus discrète de faire citer un résultat non validé."
  (should (equal (locus-artifact-badge 'chose-nouvelle) "? inconnu"))
  (should-not (equal (locus-artifact-badge 'chose-nouvelle)
                     (locus-artifact-badge 'promoted))))

(ert-deftest locus-artifact-les-six-etats-sont-ceux-du-serveur ()
  "Le client n'invente pas de vocabulaire : un état affiché qui n'existe pas
côté serveur serait une information que rien ne peut confirmer."
  (should (equal locus-artifact-states
                 '(declared uploaded quarantined verified promoted rejected))))

(ert-deftest locus-artifact-seuls-deux-etats-servent-du-contenu ()
  "`declared' n'a pas encore de contenu, `rejected' n'en a plus."
  (should (locus-artifact-servable-p 'promoted))
  (should (locus-artifact-servable-p 'verified))
  (dolist (state '(declared uploaded quarantined rejected))
    (ert-info ((format "état %s" state))
      (should-not (locus-artifact-servable-p state)))))

(ert-deftest locus-artifact-un-etat-non-servable-refuse-l-ouverture ()
  (dolist (state '(declared uploaded quarantined rejected))
    (ert-info ((format "état %s" state))
      (should-error (locus-artifact-open-plan (list (cons :state state)))
                    :type 'locus-artifact-refused))))

;; ------------------------------------------------------------------------
;; Rien n'est exécuté
;; ------------------------------------------------------------------------

(ert-deftest locus-artifact-rien-n-est-jamais-execute ()
  "§21.2 : « fichier non exécuté automatiquement ».

Un artefact vient d'une exécution non fiable, et le client est la machine de
l'utilisateur.  Le plan d'ouverture est **rendu** plutôt qu'exécuté, ce qui
permet de tester le refus sans écrire un octet sur le disque."
  (dolist (filename '("resultat.csv" "script.sh" "binaire.exe" "notes.org"))
    (ert-info ((format "fichier %s" filename))
      (let ((plan (locus-artifact-open-plan
                   (list (cons :state 'promoted) (cons :filename filename)))))
        (should (eq (alist-get :execute plan) nil))
        (should (eq (alist-get :mode plan) 'read-only))))))

(ert-deftest locus-artifact-un-type-douteux-part-en-quarantaine ()
  "§21.2 : « quarantaine si type douteux ».  Le coût d'une quarantaine indue est
une commande de plus ; celui d'un faux négatif est une exécution sur la machine
de l'utilisateur."
  (dolist (suspect '("charge.sh" "outil.exe" "greffon.el" "script.PY" "app.jar"))
    (ert-info ((format "fichier %s" suspect))
      (should (locus-artifact-suspect-p suspect))))
  (dolist (ordinaire '("mesures.csv" "figure.png" "rapport.pdf" "notes.org" nil))
    (ert-info ((format "fichier %s" ordinaire))
      (should-not (locus-artifact-suspect-p ordinaire)))))

(ert-deftest locus-artifact-la-quarantaine-ne-depend-pas-de-la-casse ()
  "Renommer en `.SH' contournerait une comparaison sensible à la casse, et
c'est le renommage le moins coûteux qui soit."
  (should (locus-artifact-suspect-p "charge.SH"))
  (should (locus-artifact-suspect-p "outil.ExE")))

(ert-deftest locus-artifact-la-quarantaine-figure-dans-le-plan ()
  "Elle ne remplace pas le refus d'exécuter : les deux sont dans le plan, et
c'est voulu — la liste d'extensions n'est pas une garantie."
  (let ((plan (locus-artifact-open-plan
               (list (cons :state 'promoted) (cons :filename "charge.sh")))))
    (should (alist-get :quarantine plan))
    (should (eq (alist-get :execute plan) nil))))

;; ------------------------------------------------------------------------
;; Le hash se vérifie avant, pas après
;; ------------------------------------------------------------------------

(ert-deftest locus-artifact-un-hash-qui-ne-correspond-pas-est-refuse ()
  "Ce qui prouve ne peut pas être ce qui est demandé : seul le hash **déclaré
avant** l'upload sert de preuve.  W6.a tient cette règle côté serveur ; le
client la tient dans le même sens."
  (let ((hasher (lambda (content) (format "sha256:%s" (length content)))))
    (should (equal (locus-artifact-verify "sha256:5" "abcde" hasher) "sha256:5"))
    (should-error (locus-artifact-verify "sha256:9" "abcde" hasher)
                  :type 'locus-artifact-refused)))

(ert-deftest locus-artifact-le-hasher-est-un-port ()
  "Le domaine ne choisit pas l'algorithme, il compare : c'est
`packages/domain' qui porte le vocabulaire de hachage, et le dupliquer ici
serait la duplication cross-repo que le `CLAUDE.md' interdit."
  (let ((appele nil))
    (locus-artifact-verify "x" "contenu" (lambda (c) (setq appele c) "x"))
    (should (equal appele "contenu"))))

;; ------------------------------------------------------------------------
;; Le paquet ne parle jamais à un runtime de containers
;; ------------------------------------------------------------------------

(ert-deftest locus-artifact-le-paquet-ne-nomme-aucun-runtime-de-containers ()
  "§20A : « le package ne parle jamais directement à Docker/Podman : toutes les
opérations passent par l'API Locus ».

Cette propriété-ci se vérifie sur le **texte** des sources, et c'est la seule
du paquet dans ce cas : un client qui contournerait le control plane ne se
trahirait pas par son comportement — il n'appellerait le runtime que sur une
machine qui en a un — mais il se trahit par ce qui est écrit.  C'est la même
forme que la frontière 4 du dépôt, appliquée au client."
  (let ((home (file-name-as-directory
               (expand-file-name (file-name-directory (locate-library "locus")))))
        (offenders nil))
    (dolist (file (directory-files home t "\\.el\\'"))
      (with-temp-buffer
        (insert-file-contents file)
        (goto-char (point-min))
        ;; Le mot est cherché comme un appel, pas comme un mot : ce fichier de
        ;; test le nomme lui-même, et une recherche naïve se prendrait elle-même.
        (when (re-search-forward
               "(\\(?:call-process\\|start-process\\|shell-command\\|process-file\\)[^)]*\\(?:docker\\|podman\\|nerdctl\\)"
               nil t)
          (push (file-name-nondirectory file) offenders))))
    (should (null offenders))))

(ert-deftest locus-artifact-le-paquet-n-execute-aucun-processus-au-chargement ()
  "Le pendant comportemental : charger le paquet ne lance rien.  Les deux
gardes se complètent — l'une lit le texte, l'autre regarde ce qui tourne."
  (should (null (process-list))))

(provide 'locus-artifact-test)

;;; locus-artifact-test.el ends here
