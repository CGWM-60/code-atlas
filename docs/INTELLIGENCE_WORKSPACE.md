# Guide de l’espace d’intelligence

## Parcours intégré

1. Analyser un dossier depuis **Projets**. La carte, les Features, les findings et les outils utilisent le même `ProjectGraph` persisté.
2. Ouvrir **Recherche** ou `Cmd/Ctrl+K` pour retrouver un symbole, un fichier, une route ou une Feature. Les scores lexical, symbolique, vectoriel, graphe et Feature expliquent le classement.
3. Demander à l’**Assistant** « Montre-moi où est gérée la connexion ». Les citations ouvrent les plages source ; les actions ouvrent les vues correspondantes. Désactiver « Ouvrir les vues automatiquement » pour choisir les actions manuellement.
4. Poursuivre avec « Quels tests manquent ? », « Il y a un risque de sécurité ? » ou « Si je rajoute du 2FA, combien de temps ? ». Le contexte sélectionné et les derniers messages orientent la recherche sans limiter celle-ci à un seul fichier.
5. Consulter **Tests**, **Estimation**, **Git** et **Dépendances** pour examiner les résultats détaillés et ouvrir les symboles associés.

Le panneau assistant se ferme, se rouvre et se redimensionne à la souris ou avec les flèches sur son séparateur. Les conversations sont persistées par projet ; elles peuvent être reprises, renommées et supprimées. La palette et les inspecteurs se ferment avec Échap.

## Recherche locale et embeddings neuronaux

La recherche locale combine le moteur exact/fuzzy existant, les termes des symboles/chemins, les relations du graphe, les Features et des vecteurs de concepts normalisés. Ces vecteurs déterministes ne sont **pas** des embeddings neuronaux.

Pour les embeddings neuronaux, configurer un provider dans les paramètres puis renseigner un **modèle d’embedding** dans Recherche. Ce modèle est distinct du modèle de chat et reste librement éditable. L’action de vectorisation transmet les unités sémantiques enrichies et expurgées au provider choisi. La recherche neuronale transmet la question ; le panneau assistant peut utiliser ce même index lorsque son option est activée.

L’index respecte les limites des symboles/fichiers/Features, sans découpage arbitraire. SQLite conserve identifiants, chemins, lignes, métadonnées, texte enrichi, résumé, hash, version et dates. `framework` peut rester inconnu : il n’est pas inventé. Le hash porte sur le contenu enrichi, donc un changement de relation ou de membership invalide également l’unité concernée.

Les vecteurs inchangés sont réutilisés. Les unités supprimées sont retirées lors de la synchronisation de l’index. Une nouvelle version de modèle possède son propre index. Chaque requête vectorise au plus 128 unités, par lots de 8 ; relancer l’action reprend les lots restants. Les unités de plus de 24 000 octets sont signalées comme ignorées ; leurs symboles enfants restent indexables. L’indexation locale est synchronisée lors de la recherche ou de la réindexation, pas par un appel réseau automatique à chaque scan.

Les adaptateurs utilisent les endpoints documentés : [OpenAI embeddings](https://developers.openai.com/api/reference/ruby/resources/embeddings/methods/create), [Mistral embeddings](https://docs.mistral.ai/api/endpoint/embeddings) et [OpenRouter embeddings](https://openrouter.ai/docs/api/api-reference/embeddings/submit-an-embedding-request). Aucun appel facturé à ces providers n’a été effectué lors de la validation de cette livraison ; les contrats applicatifs sont testés avec des providers de test.

## Assistant, contexte et preuves

`AgentOrchestrator` utilise les services de domaine existants et un registre fermé de 28 outils en lecture seule. Sans provider, les stratégies déterministes restent disponibles. Avec un provider, une planification bornée peut demander des informations supplémentaires, puis un reranker sémantique classe exclusivement les identifiants déjà retrouvés.

Les limites par défaut sont : 3 étapes d’expansion, 24 nœuds, 12 fichiers, 12 000 tokens approximatifs de contexte et 10 opérations d’outils journalisées, y compris recherche neuronale préparatoire et vérification finale. Les consultations de sources sont groupées dans l’étape de contexte ; ce compteur ne représente pas chaque lecture disque ni chaque requête de chat. Les événements WebSocket exposent les étapes publiques de travail, jamais le raisonnement privé. La réponse finale arrive en une fois ; le texte n’est pas diffusé token par token.

Les citations sont relues et vérifiées par chemin, symbole, plage et hash. Les fichiers modifiés depuis le scan ne servent pas de nouvelles preuves source avant réanalyse. L’ouverture d’une citation historique vérifie aussi son hash et refuse une preuve périmée. Cela valide l’identité de la source, **pas** toute conclusion en langage naturel : les conclusions restent explicitement à vérifier.

Les clés de provider suivent la configuration existante du navigateur et ne sont pas enregistrées dans les tables de conversations/embeddings. Des extraits source expurgés et les réponses sont conservés dans SQLite. Le filtrage des secrets réduit l’exposition, sans constituer une garantie exhaustive de détection de secrets.

## Tests, estimation et Git

**Tests** rapproche les tests des membres et relations de la Feature, propose des scénarios et permet de télécharger les tests Playwright générés. Une relation vers un test ne démontre pas ses assertions ou son exécution. La couverture fonctionnelle reste inconnue. Les fichiers générés actuellement sont des tests de navigation sur des routes statiques observées ; ils ne prétendent pas tester tout le métier ni inventer des sélecteurs.

**Estimation** affiche les Features/fichiers/symboles candidats, dépendances, routes/API, signaux de branchement, constats de sécurité, besoins de tests et inconnues. Le chiffrage comporte minimum/probable/maximum pour huit activités. Il s’agit d’un modèle heuristique non calibré sur une équipe, à réviser avant engagement.

**Git** lit la branche courante, les branches et les diffs du répertoire de travail ou de deux révisions. Les références sont validées et passées comme arguments, jamais exécutées comme texte shell. Les renommages apparaissent comme suppression/création. Les chemins UTF-8, espaces et guillemets sont décodés. Aucun index, commit ou branche n’est modifié. Les fichiers binaires/non suivis sont exclus ; le diff affiché est borné à 20 000 lignes.

Les symboles, Features, API, tests et constats associés proviennent du graphe/code actuel. Les constats introduits ou résolus entre deux anciennes révisions ne sont pas établis sans analyses historiques. Les champs Base/Cible de la revue permettent la comparaison explicite ; l’assistant reconnaît notamment « Analyse le diff entre main et feature/x ».

## Sécurité, dépendances et cache

Le moteur de taint suit certaines entrées, affectations et transmissions d’arguments sur les arêtes `CALLS` résolues, jusqu’à quatre appels. Les étapes source/appel/sink ouvrent leur propre fichier et plage. Les alias, conditions et sanitizers ne sont pas complètement résolus : ces chemins sont des candidats heuristiques, pas une preuve d’exploitabilité.

L’inventaire de dépendances lit npm, Composer, Cargo, Pub, Go et Python, ainsi que certains lockfiles, sans installer de package ni interroger un registre. Il distingue contraintes déclarées, résolutions, références locales explicites et usages trouvés dans les imports. Les constructions Pub complexes et les remplacements Go ne sont pas entièrement interprétés. Aucune base de vulnérabilités externe n’est interrogée.

Les migrations 007 et 008 sont additives et idempotentes. Les tables de conversations, messages, unités sémantiques et embeddings sont rattachées au projet par clés étrangères. Les caches AST, documentation et sorties IA existants sont conservés ; plans de tests et estimations utilisent le cache durable hash/version existant.

## API ajoutée

Les chemins ci-dessous sont relatifs à `/api/projects/{id}` :

| Méthode | Chemin | Usage UI |
| --- | --- | --- |
| GET | `/intelligence/search?q=...` | Recherche hybride et palette |
| GET / POST | `/intelligence/index` | Statistiques / réindexation locale |
| POST | `/intelligence/embeddings` | Vectorisation neuronale explicite |
| POST | `/intelligence/semantic-search` | Recherche hybride neuronale |
| GET / POST | `/conversations` | Liste / création |
| GET / PATCH / DELETE | `/conversations/{conversation}` | Historique / renommage / suppression |
| POST | `/conversations/{conversation}/messages` | Question, contexte UI, configuration optionnelle |
| GET | `/source?path=...` | Inspection d’un fichier appartenant au graphe |
| POST | `/evidence/verify` | Vérification de fraîcheur avant ouverture |
| GET | `/tests/plan?feature_id=...` | Plan projet ou Feature |
| POST | `/estimates` | Mission dans le champ `task` |
| GET | `/git/diff?base=...&head=...` | Revue du diff |
| GET | `/dependencies` | Inventaire des dépendances |

`GET /api/assistant/tools` expose les capacités affichées dans le panneau. Les événements `assistant_activity` sur `/api/events` sont filtrés par projet et conversation.

MCP conserve ses outils et ajoute `atlas_semantic_search`, `atlas_hybrid_search`, `atlas_get_test_plan`, `atlas_estimate_change`, `atlas_get_git_diff` et `atlas_review_diff`. La recherche sémantique MCP utilise l’index local de concepts ; elle ne déclenche pas de fournisseur neuronal externe.
