# RustFS Config — mémoire projet

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

M0 validé (spec, nom, périmètre). M1 fait : backend Rust `src-tauri/src/{config,error,s3,admin,backend,commands}.rs`
— clients S3 (`aws-sdk-s3`) et admin `/rustfs/admin/v3` signés SigV4 (`aws-sigv4`, service `s3`,
`x-amz-content-sha256` obligatoire), exposés par 19 commandes Tauri. Trait `RustfsBackend` pour un
futur adaptateur CLI. Test d'intégration `src-tauri/tests/admin_integration.rs` vérifié contre RustFS local.
Stockage local fait : `src-tauri/src/store.rs` (état non secret, écriture atomique) et
`src-tauri/src/secrets.rs` (trousseau OS via `keyring`, clés `rc://instance/<id>` et
`rc://credential/<id>`).

La couche Tauri est derrière la feature `gui` (défaut). Sur une machine sans GTK/WebKit :
`cargo test --no-default-features` (voir `AGENTS.md`).

## Règles à ne pas casser

- Les secrets restent dans Rust (trousseau OS), jamais dans la webview ni dans `store.json`.
- Toute modification distante suit plan → diff → apply ; apply idempotent.
- Un user IAM créé par RC est scopé à un bucket ; jamais de clé root par défaut.
- La bucket policy se fusionne par `Sid`, jamais de remplacement brut.
- Réponses en français, code et commits en anglais.

## Pièges déjà rencontrés

- Homebrew masque les outils système : son `pkg-config` ne trouve pas les `.pc` GTK et son
  `ld` (binutils) lie des libs brew incompatibles avec la glibc de Jammy. Le
  `.cargo/config.toml` local force `/usr/bin/pkg-config`, `/usr/bin/cc` et `-B/usr/bin`.
- RustFS exige `x-amz-content-sha256` sur les routes admin ; `aws-sigv4` ne l'ajoute
  qu'avec `PayloadChecksumKind::XAmzSha256`.
- `keyring` 3 : utiliser `sync-secret-service` + `crypto-rust` (et non `linux-native`,
  qui passe par le keyring du noyau et ne survit pas au redémarrage).

Voir aussi `docs/RUSTFS-NOTES.md` §6 (remplacement de bucket policy, `readonly` sans
`ListBucket`, `PUT` refusé par fermeture de connexion, SSE-S3 sans clé maître, test
d'isolation à faire sur un bucket neuf).
