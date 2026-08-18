;;; locus-dashboard-test.el --- Test de sortie de W8.d  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Un buffer se reconstruit depuis le cache, sans réseau.**
;;
;; « Sans réseau » ne se vérifie pas en débranchant la machine : le test
;; **empoisonne** les primitives réseau d'Emacs avant d'appeler le rendu.  Un
;; rendu qui parlerait à quiconque échoue alors, au lieu de réussir plus
;; lentement — et c'est la différence entre une propriété et une habitude.

;;; Code:

(require 'ert)
(require 'cl-lib)
(require 'locus-cache)
(require 'locus-dashboard)

(defmacro locus-dashboard-test--offline (&rest body)
  "Exécuter BODY avec toute sortie réseau rendue fatale."
  (declare (indent 0))
  `(cl-letf (((symbol-function 'url-retrieve)
              (lambda (&rest _) (error "le rendu a tenté un url-retrieve")))
             ((symbol-function 'url-retrieve-synchronously)
              (lambda (&rest _) (error "le rendu a tenté un url-retrieve-synchronously")))
             ((symbol-function 'make-network-process)
              (lambda (&rest _) (error "le rendu a ouvert une socket")))
             ((symbol-function 'open-network-stream)
              (lambda (&rest _) (error "le rendu a ouvert un stream"))))
     ,@body))

(defvar locus-dashboard-test--now 1000.0)

(defmacro locus-dashboard-test--with-clock (&rest body)
  "Exécuter BODY avec une horloge fixée."
  (declare (indent 0))
  `(let ((locus-cache-clock-function (lambda () locus-dashboard-test--now)))
     ,@body))

(defun locus-dashboard-test--program (name &rest options)
  "Un programme factice nommé NAME."
  (append (list (cons :name name)
                (cons :status (or (plist-get options :status) "active"))
                (cons :branches (or (plist-get options :branches) 3))
                (cons :tasks 7)
                (cons :agents 2)
                (cons :reviews 1)
                (cons :budget "12/30")
                (cons :last-event "il y a 2s")
                (cons :risk "bas"))))

(defun locus-dashboard-test--reset ()
  "Vider le cache entre deux tests."
  (locus-cache-purge)
  (setq locus-dashboard-test--now 1000.0))

;; ------------------------------------------------------------------------
;; Le rendu ne parle à personne
;; ------------------------------------------------------------------------

(ert-deftest locus-dashboard-le-buffer-se-reconstruit-sans-reseau ()
  "Le test qui porte le sprint.

Un tableau de bord qui interroge le serveur est aussi disponible que le
réseau, alors qu'il sert précisément à savoir ce qui se passe quand quelque
chose ne va pas."
  (locus-dashboard-test--reset)
  (locus-dashboard-test--with-clock
    (locus-cache-put "prg-1" (locus-dashboard-test--program "Riemann") :cursor 42)
    (locus-cache-put "prg-2" (locus-dashboard-test--program "Navier"))

    (locus-dashboard-test--offline
      (let ((buffer (locus-dashboard-render '("prg-1" "prg-2"))))
        (should (bufferp buffer))
        (with-current-buffer buffer
          (should (eq major-mode 'locus-dashboard-mode))
          (should (equal (length tabulated-list-entries) 2))
          (let ((text (buffer-substring-no-properties (point-min) (point-max))))
            (should (string-match-p "Riemann" text))
            (should (string-match-p "Navier" text))))))))

(ert-deftest locus-dashboard-le-poison-attrape-vraiment-un-appel-reseau ()
  "Le garde-fou du test précédent, éprouvé.

Sans ce cas, `--offline' pourrait ne rien empoisonner du tout et le test de
sortie passerait pour la mauvaise raison — la faute que W7.f et W7.g ont
chacune produite une fois."
  (locus-dashboard-test--offline
    (should-error (url-retrieve "http://exemple" #'ignore))
    (should-error (make-network-process :name "x"))
    (should-error (open-network-stream "x" nil "exemple" 80))))

;; ------------------------------------------------------------------------
;; Ce qui manque au cache manque à l'écran, et s'y voit
;; ------------------------------------------------------------------------

(ert-deftest locus-dashboard-une-cle-absente-ne-produit-pas-de-ligne-vide ()
  "Inventer une ligne vide laisserait croire à un programme sans activité, ce
qui est une information — et une information fausse."
  (locus-dashboard-test--reset)
  (locus-dashboard-test--with-clock
    (locus-cache-put "prg-1" (locus-dashboard-test--program "Riemann"))
    (should (equal (length (locus-dashboard-rows '("prg-1" "prg-inconnu"))) 1))))

(ert-deftest locus-dashboard-une-donnee-perimee-s-affiche-comme-telle ()
  "§22.2 : toute donnée offline affiche son état `stale'.

La péremption prend le pas sur le statut rapporté : afficher `active' sur une
donnée vieille d'un jour serait exact au moment de la lecture et faux à
l'écran, ce qui est la seule des deux choses que l'utilisateur voit."
  (locus-dashboard-test--reset)
  (locus-dashboard-test--with-clock
    (locus-cache-put "prg-1" (locus-dashboard-test--program "Riemann" :status "active"))
    (should (equal (aref (cadr (car (locus-dashboard-rows '("prg-1")))) 1) "active"))

    (setq locus-dashboard-test--now (+ 1000.0 locus-cache-ttl 1))
    (should (equal (aref (cadr (car (locus-dashboard-rows '("prg-1")))) 1) "stale"))))

(ert-deftest locus-dashboard-l-en-tete-porte-la-synchro-le-curseur-et-l-etat ()
  "Les trois de §22.2, ensemble.  Une ligne muette sur sa fraîcheur se lit
comme une ligne à jour."
  (locus-dashboard-test--reset)
  (locus-dashboard-test--with-clock
    (locus-cache-put "prg-1" (locus-dashboard-test--program "Riemann") :cursor 42)
    (setq locus-dashboard-test--now 1030.0)

    (let ((header (locus-dashboard-header '("prg-1"))))
      (should (string-match-p "30s" header))
      (should (string-match-p "42" header))
      (should-not (string-match-p "STALE" header)))

    (setq locus-dashboard-test--now (+ 1000.0 locus-cache-ttl 1))
    (should (string-match-p "STALE" (locus-dashboard-header '("prg-1"))))))

(ert-deftest locus-dashboard-l-en-tete-annonce-la-synchro-la-plus-ancienne ()
  "Quand les lignes n'ont pas le même âge, c'est la **plus ancienne** qui compte.

Annoncer la plus récente ferait passer pour frais un tableau où une seule
ligne l'est : l'en-tête résume la confiance qu'on peut accorder à l'écran
entier, et cette confiance vaut celle de sa donnée la plus vieille."
  (locus-dashboard-test--reset)
  (locus-dashboard-test--with-clock
    (locus-cache-put "vieux" (locus-dashboard-test--program "Riemann"))
    (setq locus-dashboard-test--now 1500.0)
    (locus-cache-put "frais" (locus-dashboard-test--program "Navier"))
    (setq locus-dashboard-test--now 1510.0)

    ;; Le vieux a 510 s, le frais en a 10.
    (should (string-match-p "510s" (locus-dashboard-header '("vieux" "frais"))))
    (should (string-match-p "510s" (locus-dashboard-header '("frais" "vieux")))
            )))

(ert-deftest locus-dashboard-un-cache-vide-le-dit ()
  "« Jamais synchronisé » et « synchronisé il y a 0s » ne sont pas la même
chose, et c'est la première qui explique un tableau vide."
  (locus-dashboard-test--reset)
  (locus-dashboard-test--with-clock
    (should (string-match-p "jamais" (locus-dashboard-header '("prg-1"))))
    (should (null (locus-dashboard-rows '("prg-1"))))))

;; ------------------------------------------------------------------------
;; Le cache n'est pas canonique, et ne prend pas de secrets
;; ------------------------------------------------------------------------

(ert-deftest locus-cache-une-lecture-rend-l-entree-jamais-la-valeur-nue ()
  "§21.3 : « ne sert pas de source canonique ».

La contrainte est de type, pas de documentation : dès qu'une lecture rend la
valeur nue, l'appelant suivant la traite comme un fait.  Lire le cache oblige
donc à voir de quand date ce qu'on lit."
  (locus-dashboard-test--reset)
  (locus-dashboard-test--with-clock
    (locus-cache-put "k" 'valeur)
    (let ((entry (locus-cache-get "k")))
      (should (locus-cache-entry-p entry))
      (should (eq (locus-cache-entry-value entry) 'valeur))
      (should (numberp (locus-cache-entry-stored-at entry))))))

(ert-deftest locus-cache-une-valeur-declaree-sensible-est-refusee ()
  "§21.3 : « ne contient pas les secrets ».

Refusée, pas détectée : une heuristique qui fouillerait les valeurs raterait le
premier format qu'elle ne connaît pas, tout en donnant l'impression d'avoir
vérifié.  Le cache survit à l'arrêt d'Emacs et se copie avec le répertoire — ce
qui y entre est durable, et un secret durable est un secret perdu."
  (locus-dashboard-test--reset)
  (should-error (locus-cache-put "token" "abc" :sensitive t)
                :type 'locus-cache-refused)
  (should (null (locus-cache-get "token"))))

(ert-deftest locus-cache-une-entree-perimee-est-rendue-pas-supprimee ()
  "§22.1 autorise la lecture offline : effacer au premier dépassement de TTL
priverait le mode offline de ce qu'il existe pour montrer."
  (locus-dashboard-test--reset)
  (locus-dashboard-test--with-clock
    (locus-cache-put "k" 'valeur)
    (setq locus-dashboard-test--now (+ 1000.0 locus-cache-ttl 1))
    (let ((entry (locus-cache-get "k")))
      (should entry)
      (should (locus-cache-stale-p entry))
      (should (eq (locus-cache-entry-value entry) 'valeur)))))

(ert-deftest locus-cache-se-purge-et-s-oublie ()
  "§21.3 : « est supprimable », « peut être purgé par commande »."
  (locus-dashboard-test--reset)
  (locus-dashboard-test--with-clock
    (locus-cache-put "a" 1)
    (locus-cache-put "b" 2)
    (should (equal (locus-cache-size) 2))
    (locus-cache-forget "a")
    (should (equal (locus-cache-size) 1))
    (locus-cache-purge)
    (should (equal (locus-cache-size) 0))))

(ert-deftest locus-cache-l-horloge-est-un-port ()
  "Un test de péremption qui attendrait vraiment une heure ne serait pas
exécuté ; un test qui manipule l'horloge de la machine casserait le reste de
la suite."
  (should (functionp locus-cache-clock-function)))

(provide 'locus-dashboard-test)

;;; locus-dashboard-test.el ends here
