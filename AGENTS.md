# AGENTS.md — RC (RustFS Config)

Application desktop Tauri 2 + Nuxt 4 pour configurer des instances RustFS : buckets,
policies IAM scopées, lecture publique par préfixe, lifecycle, puis génération d'un `.env`
prêt pour Laravel / Node / AWS CLI.

**Langue** : réponses en français, code, symboles, commits et documentation technique en
anglais.

## Lecture obligatoire avant de coder

1. `SPEC.md` — périmètre v1, décisions, critères d'acceptation.
2. `docs/RUSTFS-NOTES.md` — API admin `/rustfs/admin/v3`, policies, pièges vérifiés.
3. `README.md` — commandes et structure.

Toute décision qui contredit `SPEC.md` se discute d'abord, puis met à jour la spec dans le
même changement. Ne jamais élargir le périmètre sans tâche Kaneo associée.

## État du projet

Phase de spécification validée, aucun code métier. Le dépôt contient le starter
Nuxt UI / Tauri ; les tâches 1 à 14 (Kaneo) décrivent la construction. La page
`app/pages/index.vue` est encore le placeholder du starter.

## Stack

| Couche | Version |
| --- | --- |
| Tauri | 2.11.6 (`src-tauri/Cargo.toml`), CLI 2.12 |
| Rust | `rust-version` actuel 1.77.2 — **à relever**, `aws-sdk-s3` exige davantage |
| Nuxt | 4.6.2, SSR désactivé, génération statique |
| Nuxt UI | 4.11.3, Tailwind 4 |
| Bun | 1.4.2 (gestionnaire de paquets) |
| i18n | `@nuxtjs/i18n` 10.6, `i18n/locales/{en,fr}.json` |

## Commandes

```bash
bun i                     # dépendances frontend
bun run tauri:dev         # fenêtre Tauri + dev server (127.0.0.1:3000)
bun run tauri:build       # binaire de production
bun run dev               # navigateur seul, sans Tauri

cd src-tauri
cargo fmt
cargo clippy --all-targets -- -D warnings
cargo test
cargo check               # boucle rapide
```

Aucune commande ne doit toucher une instance de production sans confirmation explicite.
L'instance de test est locale : `http://127.0.0.1:9000`, credentials `lerd` / `lerdpassword`
(ou `rustfsadmin` / `rustfsadmin` sur une instance jetable).

## Architecture — règles non négociables

- **Les secrets ne quittent jamais Rust.** Clés dans le trousseau OS (`keyring`), jamais
  dans `store.json`, les logs, les rapports d'erreur ou la webview. Une commande Tauri peut
  révéler un secret une fois, après confirmation explicite.
- **Plan → diff → apply.** Aucune écriture distante depuis un formulaire. `plan` calcule
  l'état cible et le diff, `apply` exécute et renvoie un rapport par action.
- **Idempotence.** Rejouer un apply ne doit rien changer. Les ressources créées par RC sont
  marquées (`rc-` sur les policies, `state.json`), et RC ne supprime jamais une ressource
  qu'il n'a pas créée sans confirmation nommée.
- **Moindre privilège par défaut.** Un projet reçoit un user scopé à son bucket. Utiliser la
  clé admin est un choix explicite avec avertissement bloquant.
- **Une seule couche réseau.** Le backend Rust parle à S3 et à l'API admin ; le frontend
  n'appelle jamais une instance directement (pas de `fetch` vers RustFS depuis la webview).
- Erreurs Rust normalisées en `{ code, message, hint }`, affichables telles quelles.

## Conventions Rust

- Modules par domaine : `s3/`, `admin/`, `iam/`, `policy/`, `store/`, `secrets/`, `env/`.
- `thiserror` pour les erreurs typées ; pas de `unwrap()`/`expect()` dans une commande
  (seulement dans `main`/`setup` pour une erreur réellement irrécupérable).
- `serde` avec `rename_all = "camelCase"` sur les DTO exposés au frontend.
- Jamais de blocage dans un contexte async (`std::fs` → `tokio::fs`, `reqwest` async).
- Signer les requêtes admin avec `aws-sigv4` (service `s3`, path-style) ; valider tôt sur
  l'instance locale — c'est le risque technique principal du projet.
- `cargo clippy -- -D warnings` doit passer avant chaque commit.
- Tests unitaires à côté du code, tests d'intégration dans `src-tauri/tests/`.

## Conventions frontend

- Nuxt UI d'abord ; ne pas réinventer un composant qui existe (`UButton`, `UModal`,
  `UTable`, `UForm`, `UStepper`…).
- Composables `useInstances`, `useBucket` qui encapsulent `invoke()` ; aucun appel Tauri
  éparpillé dans les templates.
- Pas de Pinia en v1 : `useState`/composables suffisent. Ajouter une dépendance = demander.
- Toutes les chaînes visibles passent par `i18n/locales/*.json` ; FR et EN complets.
- Icônes Lucide (`i-lucide-*`).
- 4 espaces, single quotes, composants Vue en PascalCase, fichiers de page en kebab-case
  quand Nuxt les route.

## Sécurité Tauri

- `tauri.conf.json` : remplacer `"csp": null` par une politique stricte
  (`default-src 'self'`) — obligatoire avant toute release.
- `src-tauri/capabilities/default.json` est **beaucoup trop large** aujourd'hui
  (`fs:read-all`/`fs:write-all` sur `**/*`, `opener:allow-open-path` sur tout le disque,
  permissions `core:window:*` en bloc). Réduire au strict nécessaire : dialogue de fichier,
  écriture du `.env` choisi, presse-papiers, store.
- Pas de contenu distant chargé dans la webview, pas de `withGlobalTauri` en prod si
  inutile.
- Updater : clé de signature obligatoire avant distribution.
- Jamais de télémétrie ni d'appel réseau hors instance configurée.

## Pièges RustFS à ne pas réintroduire

Détail complet dans `docs/RUSTFS-NOTES.md` §6. Les cinq qui coûtent le plus cher :

1. `rc bucket anonymous set-json` (et l'équivalent API) **remplace toute** la bucket policy
   → toujours fusionner par `Sid` côté app.
2. `readonly` n'inclut pas `s3:ListBucket` : un bucket « invisible » n'est pas vide.
3. Un `PUT` vers un bucket non autorisé peut renvoyer une connexion fermée, pas un 403.
4. `SSE-S3` sans `RUSTFS_SSE_S3_MASTER_KEY` casse tous les PUT : ne pas activer le
   chiffrement depuis RC en v1.
5. Tester l'isolation sur un bucket **neuf et privé** ; un bucket déjà public répond
   favorablement même sans droit IAM.

## Tests

- `cargo test` pour le backend ; toute logique non triviale (fusion de policy, calcul
  d'URL `.env`, qualification de clé) a un test.
- Tests d'intégration contre une image RustFS locale épinglée, jamais contre la prod.
- Frontend : vérifier le parcours clé à la main dans `tauri:dev` ; pas de suite E2E en v1.
- Un changement de comportement met à jour `SPEC.md` (critères ou tableau d'erreurs).

## Skills installées (`.agents/skills`, collection `full-stack-skills/tauri-skills`)

Charger la skill correspondante avant de toucher le domaine concerné.

| Domaine | Skill |
| --- | --- |
| Cadrage, découpage | `tauri-app-planning` |
| IPC et commandes | `tauri-ipc`, `tauri-config`, `tauri-build` |
| Sécurité / permissions | `tauri-security`, `tauri-framework-security`, `tauri-app-plugin-permissions` |
| Store local, secrets | `tauri-app-store`, `tauri-app-stronghold` |
| Fichiers, dialogues, presse-papiers | `tauri-app-file-system`, `tauri-app-dialog`, `tauri-app-clipboard`, `tauri-app-opener` |
| Logs, erreurs | `tauri-app-logging` |
| Distribution | `tauri-app-updater`, `tauri-app-window-state`, `tauri-app-single-instance` |
| Frontend | `nuxt`, `nuxt-ui`, `vue`, `vite`, `nitro` |

`tauri-app-http-client` existe mais **ne pas l'utiliser** pour RustFS : les appels doivent
partir du backend Rust, sinon les clés transitent par la webview.

## MCP disponibles

- **`nuxt-remote`** — documentation Nuxt. À consulter avant toute question d'API Nuxt
  (routing, `useFetch`, modules, configuration).
- **`nuxt-ui`** — documentation Nuxt UI 4 (composants, props, thème). À consulter avant de
  créer un composant ou de personnaliser le thème.
- **`kaneo`** — suivi des tâches (voir ci-dessous).
- Documentation Rust/Tauri : `cargo doc`, docs.rs et `docs.rustfs.com` ; pas de MCP dédié.

Règle : doc officielle pour une API, lecture du code local pour le reste. Ne pas inventer
une prop Nuxt UI ni une signature Tauri.

## Kaneo

- Workspace : `GAiYlKA3Nr2yEh3Ea40IY0f0tvK4rjQ0`
- Projet : `wl81dyblgzt08vw666dumwmm` (`RC`)
- Colonnes (slug) : `to-do`, `in-progress`, `in-review`, `done`
- Tâches 1 → 14 : M0 validation, M1 socle (backend SigV4, secrets/store, UI), M2
  instances + explorateur, M3 IAM + isolation, M4 bucket policy + lifecycle/CORS, M5
  `.env`, M6 parcours guidé + packaging, QA.

Règles de suivi :

- Une tâche passe en `in-progress` au début du travail, en `in-review` à la PR, en `done`
  après vérification.
- Référencer le numéro de tâche dans le titre de PR et les commits (`feat(iam): … (#7)`).
- Une découverte qui change le plan se traduit par un commentaire sur la tâche concernée,
  pas par un changement silencieux de périmètre.
- Les dates butoir des tâches sont la référence de l'ordre de travail.

## Dépendances backend à ajouter (une fois, validées)

Aucune n'est encore présente ; les ajouter au moment de la tâche concernée, pas toutes d'un
coup.

| Crate | Usage |
| --- | --- |
| `aws-config`, `aws-sdk-s3` | opérations S3, signature SigV4 |
| `aws-sigv4` + `reqwest` | routes `/rustfs/admin/v3` |
| `tokio` | runtime async |
| `thiserror`, `uuid` | erreurs typées, identifiants locaux |
| `keyring` | trousseau OS |
| `time` (ou `chrono`) | horodatage du store |
| `tauri-specta` + `specta` | IPC typé, types TS générés (option recommandée) |
| `zeroize` | effacement des secrets en mémoire (option) |

Pas de nouvelle dépendance frontend sans nécessité démontrée.

## Définition de « terminé »

1. `cargo fmt`, `cargo clippy -D warnings`, `cargo test` passent.
2. Le parcours modifié fonctionne dans `tauri:dev` sur RustFS local.
3. Aucun secret en clair (store, logs, webview, presse-papiers non purgé).
4. `SPEC.md` et, si besoin, `docs/RUSTFS-NOTES.md` sont à jour.
5. La tâche Kaneo est passée en `done` avec la preuve (commande, capture ou test).
