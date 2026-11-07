# RC — mémoire projet

Point d'entrée pour les agents. Lire ce fichier, puis les documents ci-dessous selon la
tâche.

## Résumé

Application desktop Tauri 2 + Nuxt 4 (Nuxt UI 4) pour configurer des instances RustFS :
instances, buckets, policies IAM scopées, lecture publique par préfixe, lifecycle, et
génération d'un `.env` (Laravel, Node, AWS CLI).

- `AGENTS.md` — règles de travail, conventions, skills, MCP, workflow Kaneo.
- `SPEC.md` — périmètre v1, décisions d'architecture, flux, critères d'acceptation.
- `docs/RUSTFS-NOTES.md` — API admin `/rustfs/admin/v3`, policies, pièges vérifiés.
- `README.md` — commandes, structure.

## État actuel

Spec validée, aucun code métier. Le dépôt est le starter Nuxt UI / Tauri ; les tâches
Kaneo 1 à 14 portent la construction.

## Règles à ne pas casser

- Les secrets restent dans Rust (trousseau OS), jamais dans la webview ni dans `store.json`.
- Toute modification distante suit plan → diff → apply ; apply idempotent.
- Un user IAM créé par RC est scopé à un bucket ; jamais de clé root par défaut.
- La bucket policy se fusionne par `Sid`, jamais de remplacement brut.
- Réponses en français, code et commits en anglais.

## Pièges déjà rencontrés

Voir `docs/RUSTFS-NOTES.md` §6 (remplacement de bucket policy, `readonly` sans
`ListBucket`, `PUT` refusé par fermeture de connexion, SSE-S3 sans clé maître, test
d'isolation à faire sur un bucket neuf).
