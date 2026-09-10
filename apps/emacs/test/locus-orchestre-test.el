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
          (format "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"task_id\":\"%s\",\"output\":{\"summary\":\"CE QUE %s A ETABLI\"}}"
                  tache tache)
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
         (locus-orchestre-test--soumises nil))
     (locus-cache-purge)
     (setq locus-orchestre--plan nil
           locus-orchestre--courante nil
           locus-orchestre--faites nil
           locus-orchestre--nom nil)
     (when (timerp locus-orchestre--timer) (cancel-timer locus-orchestre--timer))
     (setq locus-orchestre--timer nil)
     (unwind-protect (progn ,@body)
       (when (timerp locus-orchestre--timer) (cancel-timer locus-orchestre--timer))
       (setq locus-orchestre--timer nil))))

(defun locus-orchestre-test--etapes (&rest questions)
  "Des étapes minimales pour QUESTIONS."
  (mapcar (lambda (q) (locus-etape-creer :question q :conditions '("rendue"))) questions))

;; ------------------------------------------------------------------------

(ert-deftest locus-orchestre-une-etape-part-des-le-lancement ()
  "**Le test de sortie.**  Lancer un plan soumet la première étape tout de suite.

Attendre l'intervalle ferait passer cinq secondes où rien ne se voit, et un
lancement qui ne fait rien se lit comme un lancement raté."
  (locus-orchestre-test--avec
    (locus-orchestre--avancer)                ; le tour que `lancer' fait d'emblée
    (should (null locus-orchestre--courante))  ; rien n'a été soumis : le plan est vide
    (setq locus-orchestre--plan (locus-orchestre-test--etapes "Première"))
    (locus-orchestre--avancer)
    (should locus-orchestre--courante)
    (should (equal (car locus-orchestre-test--soumises) "Première"))))

(ert-deftest locus-orchestre-la-suivante-attend-que-la-precedente-finisse ()
  "Une étape ne part pas tant que la précédente n'a pas abouti.

C'est toute la différence entre un plan et trois missions lancées ensemble :
la seconde travaille sur ce que la première a établi."
  (locus-orchestre-test--avec
    (setq locus-orchestre--plan (locus-orchestre-test--etapes "Une" "Deux"))
    (locus-orchestre--avancer)
    (let ((premiere locus-orchestre--courante))
      (should (equal (length locus-orchestre-test--soumises) 1))
      ;; Elle tourne : rien de neuf ne part.
      (setq locus-orchestre-test--journal
            (list (locus-orchestre-test--evenement premiere "run.started" 1)))
      (locus-orchestre--avancer)
      (should (equal (length locus-orchestre-test--soumises) 1))
      (should (equal locus-orchestre--courante premiere))
      ;; Elle finit : le tour la range, le suivant soumet la seconde.
      (setq locus-orchestre-test--journal
            (append locus-orchestre-test--journal
                    (list (locus-orchestre-test--evenement premiere "run.completed" 2))))
      (locus-orchestre--avancer)
      (should (null locus-orchestre--courante))
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
    (setq locus-orchestre--plan (locus-orchestre-test--etapes "Une" "Deux"))
    (locus-orchestre--avancer)
    (let ((premiere locus-orchestre--courante))
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
    (setq locus-orchestre--plan (locus-orchestre-test--etapes "Une" "Deux"))
    (locus-orchestre--avancer)
    (let ((premiere locus-orchestre--courante))
      (setq locus-orchestre-test--journal
            (list (locus-orchestre-test--evenement premiere "run.failed" 1)))
      (locus-orchestre--avancer)
      (should (null locus-orchestre--timer))
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
    (setq locus-orchestre--plan (locus-orchestre-test--etapes "Une" "Deux"))
    (locus-orchestre--avancer)
    (setq locus-orchestre-test--journal
          (list (locus-orchestre-test--evenement locus-orchestre--courante
                                                 "task.exotique" 1)))
    (locus-orchestre--avancer)
    (should locus-orchestre--courante)
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
      (should (null locus-orchestre--timer))
      ;; Aucune requête d'annulation n'est partie.
      (should (equal (length locus-orchestre-test--soumises) partie)))))

(provide 'locus-orchestre-test)

;;; locus-orchestre-test.el ends here
