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
`rc://credential/<id>`). Coquille UI faite : `app/layouts/default.vue` (sidebar instances/
buckets, bascule FR/EN), `app/pages/index.vue` (6 onglets, états vides), composables
`app/composables/useInstances.ts` et `useBucket.ts`, CSP stricte dans
`src-tauri/tauri.conf.json`. CRUD instances et explorateur buckets/objets faits :
`src-tauri/src/instances.rs` (`list/test/save/delete`), `src-tauri/src/qualify.rs` (root /
admin / limitée / invalide), `src-tauri/src/browser.rs` (résolution `instanceId` → connexion
avec lecture du trousseau), `src-tauri/src/s3.rs` (`list_objects_page` avec delimiter et
continuation token, `create_bucket`, `delete_bucket`, upload/téléchargement par chemin).
Commandes `instanceId`-scoped : `list_instance_buckets`, `list_instance_objects`,
`create_instance_bucket`, `delete_instance_bucket`, `upload_instance_object`,
`download_instance_object`, `delete_instance_object`. UI : `app/components/InstanceForm.vue`,
`app/components/BucketDialog.vue`, onglet Objets dans `app/pages/index.vue`. Les credentials
d'instance vivent au trousseau (`secrets::set_instance_credentials`), pas dans `store.json`.
Moteur IAM fait : `src-tauri/src/iam.rs` génère les policies SPEC §7.1 (`rw`, `ro`,
`rw-sans-suppression` avec `Deny`), crée `<bucket>-app`, attache
(`set-user-or-group-policy`), fait tourner le secret (`add-user` remplace le secret de la même
access key), révoque (detach → disable → remove-user, policy `rc-` supprimée seulement si plus
aucun user), et adopte une clé existante sans la modifier. Secret généré côté Rust (40
caractères), stocké au trousseau, renvoyé une seule fois. UI : `app/components/AccessPanel.vue`
(onglet Accès) + `app/composables/useCredentials.ts`. Test d'intégration
`src-tauri/tests/iam.rs` (créer → vérifier l'isolation → tourner → révoquer).

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
- RustFS renvoie la liste des noms de buckets à toute clé authentifiée, même sans
  `ListAllMyBuckets` dans sa policy. L'isolation se vérifie sur l'accès aux objets d'un
  autre bucket, pas sur les noms (SPEC §10 le tolère explicitement).
- `add-user` avec une access key existante remplace son secret : c'est la rotation. L'ancien
  secret est refusé immédiatement.

Voir aussi `docs/RUSTFS-NOTES.md` §6 (remplacement de bucket policy, `readonly` sans
`ListBucket`, `PUT` refusé par fermeture de connexion, SSE-S3 sans clé maître, test
d'isolation à faire sur un bucket neuf).
