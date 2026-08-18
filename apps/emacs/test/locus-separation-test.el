;;; locus-separation-test.el --- Test de sortie de W8.a  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Le package se charge seul, et charger ne coûte rien.**
;;
;; `docs/10' fixe ce commit en premier de W8 : « il fixe la frontière avant
;; qu'il y ait quoi que ce soit à séparer — le seul moment où c'est gratuit ».
;; La raison est que la dépendance qu'on veut interdire ne s'ajoute jamais
;; délibérément : elle s'installe le jour où une fonction du cockpit a besoin
;; d'une chose que la configuration personnelle de l'auteur fournit déjà, et
;; elle est alors invisible dans le diff.
;;
;; Deux gardes indépendantes vérifient cela, et c'est voulu.  Celle-ci, en
;; Elisp, depuis l'intérieur du package ; et la règle 5 de
;; `tooling/boundaries/', en TypeScript, depuis l'extérieur.  Elles ne
;; partagent aucun code — une garde et le test qui l'emploie bougeraient
;; ensemble, et cesseraient ensemble de garder quoi que ce soit.

;;; Code:

(require 'ert)
(require 'locus)

(defconst locus-separation-test--builtin-load-path
  (mapcar (lambda (directory)
            (file-name-as-directory (expand-file-name (or directory "."))))
          load-path)
  "La `load-path' telle qu'elle est **après** chargement du package.

Capturée à la lecture de ce fichier, donc après le `require' ci-dessus : les
tests qui la consultent constatent l'état d'arrivée, pas l'état de départ.")

;; ------------------------------------------------------------------------
;; Le package se charge seul
;; ------------------------------------------------------------------------

(ert-deftest locus-separation-le-package-se-charge-sous-emacs-Q ()
  "Le simple fait d'arriver ici le prouve : ce fichier a fait `require' de
`locus' sous un Emacs sans configuration.  Ce que le test ajoute est la
vérification que le chargement a bien produit ce qu'il annonce."
  (should (featurep 'locus))
  (should (featurep 'locus-protocol))
  (should (stringp locus-version))
  (should (stringp locus-protocol-version)))

(ert-deftest locus-separation-aucune-dependance-hors-du-paquet ()
  "Aucune bibliothèque tierce n'est requise.

Les seules `features' présentes doivent être celles d'Emacs, celles du paquet,
et celles du harnais de test.  Une dépendance tierce chargée ici serait une
dépendance que l'utilisateur devrait installer sans que rien le lui dise."
  ;; Le critère est l'**emplacement**, pas une liste de noms : une liste de noms
  ;; se met à jour à la main, donc elle finit par autoriser ce qu'on y a ajouté
  ;; pour faire passer le test.  Une `feature' est à nous quand elle vient du
  ;; répertoire du paquet, et à Emacs quand elle vient de son installation.
  (let* ((suspects (seq-remove
                    (lambda (feature)
                      (or (locus-separation-test--ours-p feature)
                          (locus-separation-test--builtin-p feature)))
                    features)))
    (should (null suspects))))

(defun locus-separation-test--ours-p (feature)
  "Renvoyer non-nil quand FEATURE vient du répertoire du paquet ou de ses tests."
  ;; Le répertoire du paquet, et lui seul : `test/' est dedans, `apps/' ne
  ;; l'est pas — remonter d'un cran de plus autoriserait n'importe quelle autre
  ;; application du monorepo à se glisser dans les dépendances du client.
  (let ((file (locus-separation-test--feature-file feature))
        (home (file-name-as-directory
               (expand-file-name (file-name-directory
                                  (locate-library "locus"))))))
    (and file (string-prefix-p home (expand-file-name file)))))

(defun locus-separation-test--builtin-p (feature)
  "Renvoyer non-nil quand FEATURE vient de l'installation d'Emacs."
  (let ((file (locus-separation-test--feature-file feature)))
    (or (null file)
        (string-prefix-p (file-name-as-directory
                          (expand-file-name data-directory))
                         file)
        (string-prefix-p (file-name-as-directory
                          (expand-file-name (file-name-directory
                                             (directory-file-name data-directory))))
                         file))))

(defun locus-separation-test--feature-file (feature)
  "Le fichier d'où FEATURE a été chargée, ou nil s'il est inconnu.

`load-history' associe un fichier à ses définitions, et une `feature' y figure
sous la forme (provide . FEATURE) parmi celles-ci — elle n'est pas la clé.  La
chercher comme une clé rend toujours nil, ce qui ferait passer n'importe quelle
dépendance tierce pour un composant d'Emacs : le test resterait vert en ne
regardant rien."
  (car (seq-find (lambda (entry)
                   (member (cons 'provide feature) (cdr entry)))
                 load-history)))

;; ------------------------------------------------------------------------
;; Charger ne coûte rien
;; ------------------------------------------------------------------------

(ert-deftest locus-separation-charger-n-ouvre-aucune-connexion ()
  "`SPEC.md' §7.1 : « ne lancer aucun stack serveur sans action explicite ».

Un client qui se connecterait au chargement rendrait le démarrage d'Emacs
dépendant d'un daemon — et c'est le critère qui prime dans le `CLAUDE.md' de
`emacs-config' : le startup reste fonctionnel sans réseau et sans que Locus
tourne."
  (should (null (process-list)))
  (should-not (locus-connected-p)))

(ert-deftest locus-separation-charger-n-arme-aucun-timer ()
  "Pas de reconnexion agressive au démarrage.

Un timer armé au chargement est la forme discrète du même défaut : rien ne se
connecte à la lecture du fichier, et tout se connecte une seconde plus tard.
Le test regarde donc les timers, pas seulement les processus.

Il regarde les timers **du paquet**, pas tous : Emacs arme les siens — celui de
`show-paren-mode', par exemple — et exiger une liste vide ferait échouer le test
sur le comportement d'Emacs plutôt que sur celui du client.  C'est la différence
entre une propriété et une coïncidence."
  (should (null (locus-separation-test--our-timers timer-list)))
  (should (null (locus-separation-test--our-timers timer-idle-list))))

(defun locus-separation-test--our-timers (timers)
  "Les TIMERS dont la fonction appartient au paquet."
  (seq-filter
   (lambda (timer)
     (let ((function (timer--function timer)))
       (and (symbolp function)
            (string-prefix-p "locus-" (symbol-name function)))))
   timers))

(ert-deftest locus-separation-charger-ne-sort-pas-du-repertoire ()
  "Le paquet n'ajoute à `load-path' aucun répertoire hors du sien.

C'est la faute que la lecture du source ne montre pas : un `add-to-list' sur
`load-path' au chargement a exactement l'air d'un paquet autonome dans un
diff."
  (let* ((home (file-name-as-directory
                (expand-file-name
                 (file-name-directory (locate-library "locus")))))
         (escapes (seq-remove
                   (lambda (directory)
                     (or (string-prefix-p home directory)
                         (locus-separation-test--builtin-directory-p directory)))
                   locus-separation-test--builtin-load-path)))
    (should (null escapes))))

(defun locus-separation-test--builtin-directory-p (directory)
  "Renvoyer non-nil quand DIRECTORY appartient à l'installation d'Emacs."
  (string-prefix-p (file-name-as-directory
                    (expand-file-name (file-name-directory
                                       (directory-file-name data-directory))))
                   directory))

;; ------------------------------------------------------------------------
;; Ce que le client sait dire sans serveur
;; ------------------------------------------------------------------------

(ert-deftest locus-separation-le-client-se-decrit-sans-reseau ()
  "Dire ce que le client est reste possible quand dire ce que le serveur est ne
l'est pas.  C'est le premier diagnostic utile quand rien ne répond."
  (let ((message-log-max nil))
    (should (string-match-p (regexp-quote locus-version) (locus-describe)))
    (should (string-match-p (regexp-quote locus-protocol-version)
                            (locus-describe)))
    (should (string-match-p "non connecté" (locus-describe)))))

;; ------------------------------------------------------------------------
;; La hiérarchie d'erreurs ne dépend pas de l'ordre de chargement
;; ------------------------------------------------------------------------

(ert-deftest locus-separation-toute-erreur-du-paquet-derive-de-error ()
  "Chaque condition `locus-*' contient `locus-error' **et** `error'.

Sans `error' dans ses conditions, une erreur échappe à tout `condition-case'
ordinaire : elle traverse les gardes qui existaient pour l'attraper.  Le défaut
est arrivé — `locus-error' vivait dans `locus-auth', et les modules qui en
héritent ne requièrent que `locus' — et il ne se voyait pas, parce que l'ordre
alphabétique des fichiers de test chargeait `locus-auth' en premier.  Une
correction qui dépend de l'ordre de chargement n'en est pas une, et ce test est
ce qui empêche la suivante d'en dépendre."
  (let (offenders)
    (mapatoms
     (lambda (symbol)
       (let ((conditions (get symbol 'error-conditions)))
         (when (and conditions
                    (string-prefix-p "locus-" (symbol-name symbol))
                    (not (eq symbol 'locus-error))
                    (or (not (memq 'error conditions))
                        (not (memq 'locus-error conditions))))
           (push symbol offenders)))))
    (should (null offenders))))

(ert-deftest locus-separation-la-racine-est-au-point-d-entree ()
  "`locus-error' est définie par `locus.el', que tout module requiert — c'est ce
qui rend la hiérarchie vraie quel que soit ce qui est chargé."
  (should (memq 'error (get 'locus-error 'error-conditions))))

(provide 'locus-separation-test)

;;; locus-separation-test.el ends here
