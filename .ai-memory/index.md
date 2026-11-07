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
Vérifications d'isolation faites : `src-tauri/src/verify.rs` exécute les contrôles SPEC §10
(`list_buckets`, `probe_write`, `probe_delete`, `private_read`, `public_read`,
`anonymous_list`, `control_bucket`, `bucket_policy`, `lifecycle`), supprime toujours la sonde
`_rc-probe/<nanos>.txt` et renvoie `Report { ok, checks[{id,status,detail}] }`. Commande
`verify_isolation` ; bouton bouclier et modale de rapport dans `app/components/AccessPanel.vue`
(clés i18n `access.verify*`, `access.checks.*`, `access.checkStatus.*`). Test
`src-tauri/tests/isolation.rs` : rapport vert puis `bucket_policy` en échec si le store
déclare une policy absente.
Bucket policy faite : `src-tauri/src/policy.rs` génère un statement `s3:GetObject` par préfixe
public (`Sid` `rc-public-<slug>`), fusionne avec la policy existante en conservant tout
statement non `rc-public-`, calcule un diff (`add`/`update`/`remove`) et ne réécrit rien si le
diff est vide. Préfixe vide (bucket entier) exige `allowRoot` + confirmation nommée. Après
apply, `public_prefixes` et `managed.bucketPolicy` sont enregistrés dans le store. Commande
`bucket_state` pour préremplir l'UI, `plan_bucket_policy`, `apply_bucket_policy` ; panneau
`app/components/PolicyPanel.vue` (onglet Policy), i18n `policy.*`. Test
`src-tauri/tests/policy.rs` : deux apply ne changent rien et un statement étranger survit.
Lifecycle/versioning/CORS faits : `src-tauri/src/lifecycle.rs` gère les règles
`rc-expire-<slug>` + purge multipart (`rc-abort-incomplete-multipart`, 7 j), fusionne en
conservant les règles étrangères, active/suspend le versioning et pose une règle CORS.
`plan_bucket_config` / `apply_bucket_config` (overwrite CORS = `cors_overwrite_confirmation`
sauf `confirmCors`), sonde `_rc-probe-lifecycle.txt` avec en-tête `Expiration` supprimée
dans tous les cas. UI : `app/components/LifecyclePanel.vue` (onglet Lifecycle), i18n
`lifecycle.*`. Test `src-tauri/tests/lifecycle.rs` : idempotence règles + versioning,
conflit CORS confirmé, aucune sonde restante.
Générateur `.env` fait : `src-tauri/src/env.rs` rend les trois formats SPEC §8 depuis un
credential du store (secret lu au trousseau) — `generate_env` renvoie le texte, `write_env_file`
écrit en 0600 et refuse d'écraser sans `allowOverwrite` (`file_exists`). `AWS_URL` suit
path-style (`{endpoint}/{bucket}`) ou virtual-host (`{scheme}://{bucket}.{host}`),
`AWS_SSL_VERIFY` n'apparaît que pour un endpoint HTTPS. UI : `app/components/EnvPanel.vue`
(onglet .env), i18n `env.*`. Test `src-tauri/tests/env.rs` : le `.env` généré sert à
uploader/lire avec la clé scopée.
Parcours guidé fait : `app/components/WizardFlow.vue` (bouton « Préparer un bucket » dans
l'en-tête) enchaîne bucket → visibilité → credentials → rétention → aperçu (diff policy +
lifecycle, commandes `rc` équivalentes) puis applique la séquence et affiche le `.env`
généré. Templates `laravel-media` (public `uploads/media/`, 180 j) et vide. Test
`src-tauri/tests/wizard.rs` : produit bucket + user + `.env`, puis un rejeu ne change rien
et un second credential est refusé.

La couche Tauri est derrière la feature `gui` (défaut). Sur une machine sans GTK/WebKit :
`cargo test --no-default-features` (voir `AGENTS.md`).

## Règles à ne pas casser

- Les secrets restent dans Rust (trousseau OS), jamais dans la webview ni dans `store.json`.
  Seule exception : le texte `.env` généré à la demande explicite de l'utilisateur.
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
- Un bucket versionné ne se supprime qu'après avoir purgé versions ET delete markers ; RustFS
  ne retire qu'une version par clé et par appel `delete-objects` (boucler jusqu'à vide).

Voir aussi `docs/RUSTFS-NOTES.md` §6 (remplacement de bucket policy, `readonly` sans
`ListBucket`, `PUT` refusé par fermeture de connexion, SSE-S3 sans clé maître, test
d'isolation à faire sur un bucket neuf).
