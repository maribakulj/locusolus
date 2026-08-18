;;; locus-integration-test.el --- Test de sortie de W8.g  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Chaque intégration absente dégrade sans casser le démarrage.**
;;
;; Les quatre règles de §4.3 sont vérifiées sur le mécanisme, une fois, plutôt
;; que sur six intégrations, six fois : écrites six fois elles seraient tenues
;; cinq fois et demie, et c'est la sixième qu'on découvre sur la machine de
;; quelqu'un qui n'a pas installé le paquet.
;;
;; Les cas emploient une `feature' qui n'existe nulle part — `locus-fantome' —
;; parce qu'un test d'absence bâti sur un vrai paquet devient un test de
;; présence le jour où quelqu'un l'installe.

;;; Code:

(require 'ert)
(require 'locus-integration)

(defun locus-integration-test--reset ()
  "Repartir d'un registre vide."
  (locus-integration-forget-all))

;; ------------------------------------------------------------------------
;; Déclarer ne charge rien
;; ------------------------------------------------------------------------

;; La sonde est une bibliothèque **présente et non chargée**.  `hexl' convient :
;; elle existe dans toute installation d'Emacs, et rien dans le cockpit ni dans
;; cette suite ne l'entraîne.  Un premier essai employait `cl-extra', que la
;; suite charge par ailleurs : les tests passaient alors quoi qu'il arrive, et
;; deux mutations sur trois restaient muettes — la même dépendance à l'ordre de
;; chargement que W8.f avait trouvée dans la hiérarchie d'erreurs.
(defconst locus-integration-test--probe 'hexl
  "Une bibliothèque présente qu'aucun autre test ne charge.")

(defun locus-integration-test--assert-probe-unloaded ()
  "Vérifier la prémisse, plutôt que la supposer.

Si la sonde venait à être chargée par ailleurs, les tests qui l'emploient
deviendraient vides sans cesser de passer.  Ils échouent donc ici, bruyamment."
  (should-not (featurep locus-integration-test--probe)))

(ert-deftest locus-integration-declarer-ne-charge-rien ()
  "§7.1 : « ne pas ralentir l'ouverture de la première frame ».

Une déclaration qui sonderait le disque ferait payer au démarrage autant que
les `require' qu'elle remplace."
  (locus-integration-test--reset)
  (locus-integration-test--assert-probe-unloaded)
  (let ((before (copy-sequence features)))
    (locus-integration-declare 'sonde locus-integration-test--probe
                               :commands '(locus-sonde-ouvrir))
    (locus-integration-declare 'fantome 'locus-fantome)
    (should (equal features before))
    (should-not (featurep locus-integration-test--probe))))

(ert-deftest locus-integration-detecter-n-est-pas-charger ()
  "La règle la moins évidente, et celle qui décide de la forme du module.

`(require FEATURE nil t)' détecterait aussi bien — et chargerait la
dépendance. « Facultatif » deviendrait alors synonyme de « chargé quand
même »."
  (locus-integration-test--reset)
  (locus-integration-test--assert-probe-unloaded)
  (locus-integration-declare 'sonde locus-integration-test--probe)
  (should (locus-integration-available-p 'sonde))
  (ert-info ("la détection a répondu oui sans rien évaluer")
    (should-not (featurep locus-integration-test--probe))))

;; ------------------------------------------------------------------------
;; Une intégration absente dégrade
;; ------------------------------------------------------------------------

(ert-deftest locus-integration-une-dependance-absente-n-est-pas-disponible ()
  (locus-integration-test--reset)
  (locus-integration-declare 'fantome 'locus-fantome)
  (should-not (locus-integration-available-p 'fantome)))

(ert-deftest locus-integration-une-dependance-absente-n-ajoute-aucune-commande ()
  "§4.3 : « ajoute ses commandes seulement si disponible ».

Une commande offerte puis défaillante est pire qu'une commande absente : elle
se découvre au moment où on en a besoin."
  (locus-integration-test--reset)
  (locus-integration-declare 'fantome 'locus-fantome :commands '(locus-fantome-ouvrir))
  (should (null (locus-integration-commands 'fantome)))
  (ert-info ("ce qu'elle apporterait reste consultable, pour documenter")
    (should (equal (locus-integration-commands-declared 'fantome)
                   '(locus-fantome-ouvrir)))))

(ert-deftest locus-integration-une-dependance-presente-ajoute-ses-commandes ()
  "Sans ce cas, « n'ajoute rien quand absente » pourrait vouloir dire
« n'ajoute jamais rien »."
  (locus-integration-test--reset)
  (locus-integration-declare 'presente locus-integration-test--probe
                             :commands '(locus-presente-ouvrir))
  (should (equal (locus-integration-commands 'presente) '(locus-presente-ouvrir))))

(ert-deftest locus-integration-l-erreur-dit-quoi-installer ()
  "§4.3 : « produit une erreur **actionnable** ».

Un message qui dit seulement « indisponible » oblige à lire le code pour
savoir quoi installer, et c'est le moment où l'utilisateur en a le moins
envie."
  (locus-integration-test--reset)
  (locus-integration-declare 'fantome 'locus-fantome :package 'locus-fantome-mode)
  (let ((message (cadr (should-error (locus-integration-require 'fantome)
                                     :type 'locus-integration-missing))))
    (should (string-match-p "locus-fantome-mode" message))
    (ert-info ("le nom du paquet, pas celui de la feature : c'est lui qu'on installe")
      (should (string-match-p "installé" message)))))

(ert-deftest locus-integration-une-integration-inconnue-se-distingue-d-une-absente ()
  "Confondre les deux ferait conseiller d'installer un paquet à qui a fait une
faute de frappe."
  (locus-integration-test--reset)
  (let ((message (cadr (should-error (locus-integration-require 'jamais-declaree)
                                     :type 'locus-integration-missing))))
    (should (string-match-p "inconnue" message))))

;; ------------------------------------------------------------------------
;; Rien ne casse le démarrage
;; ------------------------------------------------------------------------

(ert-deftest locus-integration-declarer-six-absences-ne-casse-rien ()
  "Le test qui porte le sprint : toutes les intégrations de §15 à §20 déclarées,
aucune installée, et le paquet reste utilisable.

C'est la situation d'un utilisateur qui installe le cockpit sans rien d'autre —
la plus fréquente, et celle où un `require' malheureux se paierait au
démarrage."
  (locus-integration-test--reset)
  (dolist (name '(org magit xiiif jupyter eat denote))
    (locus-integration-declare name (intern (format "locus-fantome-%s" name))
                               :commands (list (intern (format "locus-%s-ouvrir" name)))))

  (should (equal (locus-integration-names) '(denote eat jupyter magit org xiiif)))
  (dolist (name (locus-integration-names))
    (ert-info ((format "intégration %s" name))
      (should-not (locus-integration-available-p name))
      (should (null (locus-integration-commands name)))
      (should-error (locus-integration-require name) :type 'locus-integration-missing)))

  (ert-info ("et rien n'a été chargé, lancé ni armé")
    (should (null (process-list)))
    (should-not (featurep 'locus-fantome-magit))))

(provide 'locus-integration-test)

;;; locus-integration-test.el ends here
