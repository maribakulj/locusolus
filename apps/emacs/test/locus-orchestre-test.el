;;; locus-orchestre-test.el --- Ce qu'un plan doit tenir  -*- lexical-binding: t; -*-

;; SPDX-License-Identifier: Apache-2.0

;;; Commentary:

;; **Plusieurs missions s'enchaînent sans qu'on soumette chaque étape.**
;;
;; Le transport est remplacé, comme partout ailleurs dans cette suite : ce qui
;; est éprouvé est l'enchaînement, pas le réseau.  Le minuteur, lui, n'est
;; jamais armé — chaque test appelle `locus-orchestre--avancer' à la main, ce
;; qui rend l'avancement **observable pas à pas** au lieu de dépendre d'une
;; horloge.  Un test qui attendrait cinq secondes pour voir une étape passer
;; serait lent et intermittent, deux défauts pour une garantie moindre.

;;; Code:

(require 'ert)
(require 'cl-lib)
(require 'locus)
(require 'locus-cache)
(require 'locus-session)
(require 'locus-mission)
(require 'locus-orchestre)

(defvar locus-orchestre-test--journal nil
  "Les événements que le faux daemon rend, dans l'ordre.")

(defvar locus-orchestre-test--soumises nil
  "Les questions soumises, la plus récente en tête.")

(defvar locus-orchestre-test--resultats-connus nil
  "Les tâches pour lesquelles le faux daemon sert un résultat.")

(defvar locus-orchestre-test--cout "0"
  "Ce que le faux daemon déclare comme coût, en texte JSON.")

(defun locus-orchestre-test--daemon (_host _port payload)
  "Un daemon qui accepte tout et retient les questions."
  (cond
   ((string-match-p "\\`GET /timeline" payload)
    (format "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"items\":[%s],\"next\":null}"
            (mapconcat #'identity locus-orchestre-test--journal ",")))
   ;; `GET /tasks/{id}/result` : ce que l'étape précédente a rendu, ou 404 tant
   ;; qu'elle n'a rien rendu.  Les deux comptent — c'est la différence entre un
   ;; relais qui porte du contenu et un relais vide.
   ((string-match "\\`GET /tasks/\\([^/?]+\\)/result" payload)
    (let ((tache (match-string 1 payload)))
      (if (member tache locus-orchestre-test--resultats-connus)
          (format "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"task_id\":\"%s\",\"output\":{\"summary\":\"CE QUE %s A ETABLI\",\"budget_spent\":{\"cost\":%s}}}"
                  tache tache locus-orchestre-test--cout)
        "HTTP/1.1 404 Not Found\r\n\r\n")))
   ((string-match-p "\\`GET " payload)
    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"items\":[],\"next\":null}")
   ((string-match-p "context-view/build" payload)
    (concat "HTTP/1.1 201 Created\r\nContent-Type: application/json\r\n\r\n"
            "{\"view\":{\"id\":\"ctx_T\",\"hash\":\"h\"}}"))
   (t
    (when (string-match "\"statement\":\"\\([^\"]*\\)\"" payload)
      ;; Repasser aux octets puis décoder, comme `locus-mission-test' le fait et
      ;; pour la même raison : `json-serialize' rend une chaîne **unibyte**, où
      ;; un accent occupe déjà ses deux octets, et la lire telle quelle donnerait
      ;; une chaîne qui ne s'égale pas à celle qu'on a écrite.
      (push (decode-coding-string (string-to-unibyte (match-string 1 payload)) 'utf-8)
            locus-orchestre-test--soumises))
    "HTTP/1.1 202 Accepted\r\n\r\n")))

(defun locus-orchestre-test--evenement (task-id type position)
  "Un événement JSON, tel que `/timeline' le rend."
  (format "{\"position\":%d,\"event_type\":\"%s\",\"stream_id\":\"task/%s\"}"
          position type task-id))

(defmacro locus-orchestre-test--avec (&rest body)
  "Exécuter BODY sur un orchestrateur neuf et un daemon simulé."
  (declare (indent 0))
  `(let ((locus-http-send-function #'locus-orchestre-test--daemon)
         (locus-endpoint "http://127.0.0.1:8787")
         (locus-mission-project "prj_T")
         (locus-orchestre-test--journal nil)
         (locus-orchestre-test--resultats-connus nil)
         (locus-orchestre-test--cout "0")
         (locus-orchestre-test--soumises nil))
     (locus-cache-purge)
     (setq locus-orchestre--arrete nil
           locus-orchestre--derniere-volee nil
           locus-orchestre--volee-echouee nil
           locus-orchestre--plan nil
           locus-orchestre--courantes nil
           locus-orchestre--faites nil
           locus-orchestre--depense 0.0
           locus-orchestre--nom nil)
     (when (timerp locus-orchestre--timer) (cancel-timer locus-orchestre--timer))
     (setq locus-orchestre--timer nil)
     (unwind-protect (progn ,@body)
       (when (timerp locus-orchestre--timer) (cancel-timer locus-orchestre--timer))
       (setq locus-orchestre--timer nil))))

(defun locus-orchestre-test--etapes (&rest questions)
  "Des étapes minimales pour QUESTIONS."
  (mapcar (lambda (q) (locus-etape-creer :question q :conditions '("rendue"))) questions))

(defun locus-orchestre-test--volees (&rest questions)
  "Un plan d'une étape par volée — la forme séquentielle, explicitée."
  (mapcar #'list (apply #'locus-orchestre-test--etapes questions)))

;; ------------------------------------------------------------------------

(ert-deftest locus-orchestre-une-etape-part-des-le-lancement ()
  "**Le test de sortie.**  Lancer un plan soumet la première étape tout de suite.

Attendre l'intervalle ferait passer cinq secondes où rien ne se voit, et un
lancement qui ne fait rien se lit comme un lancement raté."
  (locus-orchestre-test--avec
    ;; Par `lancer', pour éprouver ce que fait la commande plutôt qu'une
    ;; reconstitution de ses effets.
    (locus-orchestre-lancer "un" (locus-orchestre-test--etapes "Première"))
    (should locus-orchestre--courantes)
    (should (equal (car locus-orchestre-test--soumises) "Première"))))

(ert-deftest locus-orchestre-un-plan-vide-se-termine-et-ne-repart-pas ()
  "Un plan sans étape restante se termine, et un tour de plus ne le ranime pas.

Le drapeau d'arrêt vaut pour tous les arrêts, pas seulement pour celui qu'on
demande à la main : un minuteur qui bat encore une fois après la fin du plan ne
doit rien soumettre."
  (locus-orchestre-test--avec
    (setq locus-orchestre--arrete nil)
    (locus-orchestre--avancer)
    (should locus-orchestre--arrete)
    (setq locus-orchestre--plan (locus-orchestre-test--volees "Tardive"))
    (locus-orchestre--avancer)
    (should (null locus-orchestre--courantes))
    (should (null locus-orchestre-test--soumises))))

(ert-deftest locus-orchestre-la-suivante-attend-que-la-precedente-finisse ()
  "Une étape ne part pas tant que la précédente n'a pas abouti.

C'est toute la différence entre un plan et trois missions lancées ensemble :
la seconde travaille sur ce que la première a établi."
  (locus-orchestre-test--avec
    (setq locus-orchestre--plan (locus-orchestre-test--volees "Une" "Deux"))
    (locus-orchestre--avancer)
    (let ((premiere (car (car locus-orchestre--courantes))))
      (should (equal (length locus-orchestre-test--soumises) 1))
      ;; Elle tourne : rien de neuf ne part.
      (setq locus-orchestre-test--journal
            (list (locus-orchestre-test--evenement premiere "run.started" 1)))
      (locus-orchestre--avancer)
      (should (equal (length locus-orchestre-test--soumises) 1))
      (should (equal (car (car locus-orchestre--courantes)) premiere))
      ;; Elle finit : le tour la range, le suivant soumet la seconde.
      (setq locus-orchestre-test--journal
            (append locus-orchestre-test--journal
                    (list (locus-orchestre-test--evenement premiere "run.completed" 2))))
      (locus-orchestre--avancer)
      (should (null locus-orchestre--courantes))
      (setq locus-orchestre-test--resultats-connus (list premiere))
      (locus-orchestre--avancer)
      (should (equal (length locus-orchestre-test--soumises) 2))
      ;; Et la seconde porte **ce que la première a établi**, pas son
      ;; identifiant.  La première rédaction passait « la tâche X, achevée » :
      ;; l'étape suivante repartait à vide et répondait à côté, ce qui se
      ;; mesure sur un vrai daemon et pas dans un test qui vérifie un id.
      (should (string-match-p "Deux" (car locus-orchestre-test--soumises)))
      (should (string-match-p (format "CE QUE %s A ETABLI" premiere)
                              (car locus-orchestre-test--soumises))))))

(ert-deftest locus-orchestre-un-resultat-illisible-n-arrete-pas-le-plan ()
  "Un relais vide vaut mieux qu'un plan bloqué.

Le daemon répond 404 tant qu'aucun attempt n'a abouti, et une sortie peut ne
porter aucune prose.  L'étape suivante part alors sans relais : elle travaille
moins bien, et c'est mieux qu'un plan qui s'arrête sur une lecture manquante."
  (locus-orchestre-test--avec
    (setq locus-orchestre--plan (locus-orchestre-test--volees "Une" "Deux"))
    (locus-orchestre--avancer)
    (let ((premiere (car (car locus-orchestre--courantes))))
      (setq locus-orchestre-test--journal
            (list (locus-orchestre-test--evenement premiere "run.completed" 1)))
      ;; `resultats-connus' reste vide : le daemon rendra 404.
      (locus-orchestre--avancer)
      (locus-orchestre--avancer)
      (should (equal (length locus-orchestre-test--soumises) 2))
      (should (equal (car locus-orchestre-test--soumises) "Deux")))))

(ert-deftest locus-orchestre-un-echec-arrete-le-plan ()
  "Enchaîner après un échec ferait travailler la suite sur du vide.

Et le budget paierait chaque étape suivante pour rien — c'est la raison qui
compte, plus que la propreté du graphe."
  (locus-orchestre-test--avec
    (setq locus-orchestre--plan (locus-orchestre-test--volees "Une" "Deux"))
    (locus-orchestre--avancer)
    (let ((premiere (car (car locus-orchestre--courantes))))
      (setq locus-orchestre-test--journal
            (list (locus-orchestre-test--evenement premiere "run.failed" 1)))
      (locus-orchestre--avancer)
      (should locus-orchestre--arrete)
      ;; La seconde n'est jamais partie.
      (should (equal (length locus-orchestre-test--soumises) 1))
      ;; Et le plan garde ce qu'il reste à faire, plutôt que de l'effacer :
      ;; on doit pouvoir lire ce qui n'a pas eu lieu.
      (should (equal (length locus-orchestre--plan) 1)))))

(ert-deftest locus-orchestre-un-etat-inconnu-ne-fait-pas-avancer ()
  "Un type d'événement que ce client ne connaît pas laisse l'étape en cours.

`locus-session--missions' rend le type brut quand il ne le traduit pas.  Le
traiter comme une fin ferait soumettre la suite pendant que la précédente
tourne encore — deux missions concurrentes sur le même budget."
  (locus-orchestre-test--avec
    (setq locus-orchestre--plan (locus-orchestre-test--volees "Une" "Deux"))
    (locus-orchestre--avancer)
    (setq locus-orchestre-test--journal
          (list (locus-orchestre-test--evenement (car (car locus-orchestre--courantes))
                                                 "task.exotique" 1)))
    (locus-orchestre--avancer)
    (should locus-orchestre--courantes)
    (should (equal (length locus-orchestre-test--soumises) 1))))

(ert-deftest locus-orchestre-deux-plans-ne-tournent-pas-ensemble ()
  "Deux plans partageraient le budget et la file sans le savoir.

C'est le genre de concurrence qu'on découvre sur une facture."
  (locus-orchestre-test--avec
    (locus-orchestre-lancer "premier" (locus-orchestre-test--etapes "Une"))
    (should-error (locus-orchestre-lancer "second" (locus-orchestre-test--etapes "Deux"))
                  :type 'user-error)))

(ert-deftest locus-orchestre-un-plan-vide-est-refuse-avant-le-reseau ()
  "Le refus vient d'ici, et rien ne part."
  (locus-orchestre-test--avec
    (should-error (locus-orchestre-lancer "vide" nil) :type 'user-error)
    (should (null locus-orchestre-test--soumises))))

(ert-deftest locus-orchestre-arreter-ne-touche-pas-la-mission-en-vol ()
  "Arrêter l'orchestration et arrêter une mission sont deux gestes.

La mission a un bail chez le daemon ; la retirer d'ici laisserait un worker
travailler pour un plan qui n'existe plus, sans que personne le sache."
  (locus-orchestre-test--avec
    (locus-orchestre-lancer "un" (locus-orchestre-test--etapes "Une"))
    (let ((partie (length locus-orchestre-test--soumises)))
      (locus-orchestre-arreter "à la main")
      (should locus-orchestre--arrete)
      ;; Aucune requête d'annulation n'est partie.
      (should (equal (length locus-orchestre-test--soumises) partie)))))

(ert-deftest locus-orchestre-le-plafond-de-plan-arrete-la-suite ()
  "**Dix étapes dans leur budget peuvent ruiner un plan.**

Chaque mission porte le sien, et le worker l'oppose : au plafond, la session
s'arrête.  Mais une mission ne sait rien du plan qui l'a soumise — dix étapes à
un demi-dollar respectent chacune leur borne et en dépensent cinq.  Le plafond
d'ensemble se tient donc du côté qui voit la suite."
  (locus-orchestre-test--avec
    (let ((locus-orchestre-budget-total 1.0)
          (locus-orchestre-test--cout "0.6"))
      (setq locus-orchestre--plan (locus-orchestre-test--volees "Une" "Deux" "Trois"))
      (locus-orchestre--avancer)
      (let ((premiere (car (car locus-orchestre--courantes))))
        (setq locus-orchestre-test--resultats-connus (list premiere)
              locus-orchestre-test--journal
              (list (locus-orchestre-test--evenement premiere "run.completed" 1)))
        (locus-orchestre--avancer)
        ;; 0,6 sur 1,0 : sous le plafond, le plan continue.
        (should (< (abs (- locus-orchestre--depense 0.6)) 0.001))
        (should-not locus-orchestre--arrete)
        (locus-orchestre--avancer)
        (let ((deuxieme (car (car locus-orchestre--courantes))))
          (should deuxieme)
          (setq locus-orchestre-test--resultats-connus (list premiere deuxieme)
                locus-orchestre-test--journal
                (append locus-orchestre-test--journal
                        (list (locus-orchestre-test--evenement deuxieme "run.completed" 2))))
          (locus-orchestre--avancer)
          ;; 1,2 sur 1,0 : le plan s'arrête, et la troisième ne part pas.
          (should (>= locus-orchestre--depense 1.0))
          (should locus-orchestre--arrete)
          (should (equal (length locus-orchestre-test--soumises) 2))
          ;; Ce qui n'a pas eu lieu reste lisible.
          (should (equal (length locus-orchestre--plan) 1)))))))

(ert-deftest locus-orchestre-un-cout-illisible-vaut-zero-et-se-voit ()
  "Un worker qui ne rapporte pas sa dépense ne bloque pas le plan.

Sous-compter fait dépasser le plafond ; sur-compter arrêterait un plan qui
avait de quoi continuer.  Aucun des deux n'est bon, et c'est pourquoi le cumul
s'écrit au journal à chaque étape plutôt que d'être seulement vérifié."
  (locus-orchestre-test--avec
    (setq locus-orchestre--plan (locus-orchestre-test--volees "Une" "Deux"))
    (locus-orchestre--avancer)
    (let ((premiere (car (car locus-orchestre--courantes))))
      ;; `resultats-connus' reste vide : 404, donc aucun coût lisible.
      (setq locus-orchestre-test--journal
            (list (locus-orchestre-test--evenement premiere "run.completed" 1)))
      (locus-orchestre--avancer)
      (should (= locus-orchestre--depense 0.0))
      (should-not locus-orchestre--arrete))))

(ert-deftest locus-orchestre-une-volee-part-en-entier ()
  "**Le test de sortie du collectif.**  Trois étapes d'une volée partent ensemble.

Un plan avançait une étape à la fois : trois workers attestés attendaient donc
à deux contre un.  Une volée est ce qui les fait travailler en même temps."
  (locus-orchestre-test--avec
    (locus-orchestre-lancer
     "collectif"
     (list (apply #'locus-orchestre-test--etapes '("Facette A" "Facette B" "Facette C"))))
    (should (= (length locus-orchestre--courantes) 3))
    (should (= (length locus-orchestre-test--soumises) 3))
    ;; Trois identifiants distincts : une volée n'est pas trois fois la même tâche.
    (should (= (length (delete-dups (mapcar #'car locus-orchestre--courantes))) 3))))

(ert-deftest locus-orchestre-la-volee-suivante-attend-toute-la-precedente ()
  "Une volée ne part pas tant qu'il reste une étape en vol dans la précédente.

C'est la différence entre un collectif et une rafale : la synthèse ne doit pas
partir sur deux tiers du travail."
  (locus-orchestre-test--avec
    (locus-orchestre-lancer
     "deux volées"
     (list (apply #'locus-orchestre-test--etapes '("A" "B"))
           (car (locus-orchestre-test--etapes "Synthèse"))))
    (let ((ids (mapcar #'car locus-orchestre--courantes)))
      (should (= (length ids) 2))
      ;; La première retombe, la seconde vole encore : rien de neuf ne part.
      (setq locus-orchestre-test--journal
            (list (locus-orchestre-test--evenement (nth 0 ids) "run.completed" 1)))
      (locus-orchestre--avancer)
      (should (= (length locus-orchestre--courantes) 1))
      (should (= (length locus-orchestre-test--soumises) 2))
      ;; La seconde retombe : la volée est vide, la suivante peut partir.
      (setq locus-orchestre-test--journal
            (append locus-orchestre-test--journal
                    (list (locus-orchestre-test--evenement (nth 1 ids) "run.completed" 2))))
      (locus-orchestre--avancer)
      (should (null locus-orchestre--courantes))
      (locus-orchestre--avancer)
      (should (= (length locus-orchestre-test--soumises) 3)))))

(ert-deftest locus-orchestre-la-synthese-recoit-toute-la-volee ()
  "Ce que plusieurs agents ont établi ne vaut que réuni.

Ne passer que le dernier résultat ferait perdre le travail des autres — et
c'est précisément ce qu'on a payé pour obtenir."
  (locus-orchestre-test--avec
    (locus-orchestre-lancer
     "réunion"
     (list (apply #'locus-orchestre-test--etapes '("A" "B"))
           (car (locus-orchestre-test--etapes "Synthèse"))))
    (let ((ids (mapcar #'car locus-orchestre--courantes)))
      (setq locus-orchestre-test--resultats-connus ids
            locus-orchestre-test--journal
            (list (locus-orchestre-test--evenement (nth 0 ids) "run.completed" 1)
                  (locus-orchestre-test--evenement (nth 1 ids) "run.completed" 2)))
      (locus-orchestre--avancer)      ; range la volée
      (locus-orchestre--avancer)      ; soumet la synthèse
      (let ((question (car locus-orchestre-test--soumises)))
        (should (string-match-p "Synthèse" question))
        ;; Les **deux** contributions y sont, pas seulement la dernière.
        (dolist (id ids)
          (should (string-match-p (format "CE QUE %s A ETABLI" id) question)))))))

(ert-deftest locus-orchestre-un-echec-attend-que-la-volee-retombe ()
  "Couper pendant qu'une volée vole laisserait des missions tourner pour rien.

Elles ont un bail chez le daemon et leur coût continuerait de courir, sans que
plus personne ne le compte."
  (locus-orchestre-test--avec
    (locus-orchestre-lancer
     "échec en volée"
     (list (apply #'locus-orchestre-test--etapes '("A" "B"))
           (car (locus-orchestre-test--etapes "Jamais"))))
    (let ((ids (mapcar #'car locus-orchestre--courantes)))
      ;; La première échoue ; la seconde vole encore.
      (setq locus-orchestre-test--journal
            (list (locus-orchestre-test--evenement (nth 0 ids) "run.failed" 1)))
      (locus-orchestre--avancer)
      (should-not locus-orchestre--arrete)
      (should (= (length locus-orchestre--courantes) 1))
      ;; Elle retombe : le plan s'arrête, et la troisième étape ne part pas.
      (setq locus-orchestre-test--journal
            (append locus-orchestre-test--journal
                    (list (locus-orchestre-test--evenement (nth 1 ids) "run.completed" 2))))
      (locus-orchestre--avancer)
      (should locus-orchestre--arrete)
      (should (= (length locus-orchestre-test--soumises) 2)))))

(provide 'locus-orchestre-test)

;;; locus-orchestre-test.el ends here
