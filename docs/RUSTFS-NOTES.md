# Notes RustFS — API admin, policies, pièges

Relevé le 07/11/2026 sur RustFS (instance locale `lerd-rustfs` et instance de production
`https://storage.applix.fr`), `rc` 0.1.37. Sert de référence pour l'implémentation du
backend RustFS Config.

## 1. Pas de SDK client Rust officiel

- `docs.rustfs.com/en/developer/sdk/rust` ne publie pas de crate client : il documente
  l'usage de `aws-sdk-s3` avec un endpoint custom (`force_path_style(true)`,
  `Region::new("us-east-1")`). Rust 1.78+ requis.
- Les crates `rustfs-*` sur crates.io (1.0.1) sont des composants **serveur**
  (ecstore, filemeta, credentials…), pas un client.
- L'API admin est une API REST maison sous `/rustfs/admin/v3`, signée SigV4.

Client minimal validé par la doc officielle (`aws-config = "1"`, `aws-sdk-s3 = "1"`,
`tokio`, `anyhow`) :

```rust
let credentials = Credentials::new(access_key, secret_key, None, None, "rustfs");
let shared = aws_config::defaults(BehaviorVersion::latest())
    .region(Region::new("us-east-1"))
    .credentials_provider(credentials)
    .endpoint_url(endpoint)
    .load()
    .await;

// RustFS sert en path-style par défaut
let client = Client::from_conf(
    aws_sdk_s3::config::Builder::from(&shared).force_path_style(true).build(),
);
```

Précisions utiles pour RustFS Config :

- API S3 sur le port **9000**, console sur **9001**.
- Sans credentials définis à l'installation, le serveur local tombe sur
  `rustfsadmin` / `rustfsadmin` — à signaler comme configuration non sûre.
- Le virtual-host style exige `RUSTFS_SERVER_DOMAINS` côté serveur ; sinon le toggle
  « path-style » de RustFS Config restera le seul mode fonctionnel.
- Presigned URLs et multipart passent par le même SDK ; vérifier les exceptions listées
  dans `/reference/s3-compatibility` avant de les exposer dans l'UI.

## 2. Endpoints admin v3

Préfixe : `/rustfs/admin/v3`. Signature SigV4 avec la même clé que l'API S3.
Chaque opération exige une action `admin:*` (voir §4).

Signature vérifiée côté Rust (M1) : `aws-sigv4` avec `name("s3")`, `SignableBody::Bytes`
et **impérativement** `SigningSettings { payload_checksum_kind: PayloadChecksumKind::XAmzSha256 }`.
Sans cet en-tête, RustFS répond `InvalidRequest: missing header: x-amz-content-sha256`
avant même de vérifier les droits.

### Utilisateurs

| Opération | Méthode et chemin |
| --- | --- |
| Créer / mettre à jour | `PUT add-user?accessKey=<name>` — body `{"secretKey":"…","status":"enabled"}` |
| Lister | `GET list-users` |
| Détail | `GET user-info?accessKey=<name>` |
| Activer / désactiver | `PUT set-user-status?accessKey=<name>&status=enabled\|disabled` |
| Supprimer | `DELETE remove-user?accessKey=<name>` |
| Export / import IAM | `GET export-iam` / `PUT import-iam` |

### Groupes

| Opération | Méthode et chemin |
| --- | --- |
| Lister | `GET groups` |
| Détail | `GET group?group=<name>` |
| Membres | `PUT update-group-members` — body `{"group":"…","members":["…"],"isRemove":false}` |
| Statut | `PUT set-group-status?group=<name>&status=enabled\|disabled` |
| Supprimer | `DELETE group/<name>` (refusé si non vide) |

### Policies

| Opération | Méthode et chemin |
| --- | --- |
| Lister | `GET list-canned-policies` |
| Détail | `GET info-canned-policy?name=<policy>` |
| Créer / remplacer | `PUT add-canned-policy?name=<policy>` — body = document JSON |
| Supprimer | `DELETE remove-canned-policy?name=<policy>` |
| Attacher (remplace) | `PUT set-user-or-group-policy?policyName=&userOrGroup=&isGroup=` |
| Attacher (ajoute) | `POST idp/builtin/policy/attach` — body `{"policies":["…"],"user":"…"}` |
| Détacher | `POST idp/builtin/policy/detach` — même body |
| Mappings | `GET idp/builtin/policy-entities` |

### Divers

- Service accounts : `add-service-account`, list/delete associés.
- STS : `sts:AssumeRole`.

## 3. Policies

- Format : `{"Version":"2012-10-17","Statement":[…]}`. Seule cette version est acceptée.
- `Effect` : `Allow` / `Deny` — **un Deny explicite gagne toujours**.
- Ressources : `arn:aws:s3:::bucket` (bucket) ou `arn:aws:s3:::bucket/prefix/*` (objets).
- Actions : `s3:*`, `admin:*`, `sts:AssumeRole`, `kms:*`.
- Conditions : `StringLike`, `StringEquals`, `IpAddress`, `Bool`, `Date*`, `Numeric*`,
  suffixe `IfExists`.

### Policies intégrées

| Nom | Contenu |
| --- | --- |
| `readwrite` | `s3:*` sur tout + `sts:AssumeRole` |
| `readonly` | `s3:GetBucketLocation`, `s3:GetObject`, `s3:GetBucketQuota` — **pas** `s3:ListBucket` |
| `writeonly` | `s3:PutObject` |
| `diagnostics` | actions `admin:Profiling`, `ServerTrace`, `ConsoleLog`… |
| `consoleAdmin` | `admin:*` + `kms:*` + `s3:*` |

Conséquence : ne jamais attacher `readonly` en croyant obtenir un bucket navigable.
Toujours générer une policy scopée (voir SPEC §7.1).

## 4. Action `admin:*` par opération

`admin:CreateUser`, `ListUsers`, `GetUser`, `DeleteUser`, `EnableUser`/`DisableUser`,
`CreatePolicy`, `DeletePolicy`, `GetPolicy`, `AttachUserOrGroupPolicy`,
`ListGroups`, `GetGroup`, `AddUserToGroup`, `RemoveUserFromGroup`, `EnableGroup`,
`DisableGroup`, `ServerInfo`, etc. Une clé applicative n'a aucune de ces actions : c'est
le discriminant utilisé par RustFS Config pour désactiver les écrans IAM.

## 5. Correspondance `rc`

| Besoin | Commande |
| --- | --- |
| Alias | `rc alias set <alias> <endpoint> <key> <secret>` |
| Buckets | `rc bucket list <alias>/`, `rc bucket create`, `rc bucket remove` |
| Objets | `rc object list <alias>/<bucket>`, `rc put`, `rc get`, `rc rm` |
| Policy IAM | `rc admin policy create <alias> <name> <fichier.json>`, `attach`, `detach`, `info`, `entities`, `ls`, `rm` |
| User | `rc admin user add <alias> <name> <secret>`, `info`, `ls`, `disable`, `enable`, `rm` |
| Groupes | `rc admin group …` |
| Policy anonyme | `rc bucket anonymous set-json <alias>/<bucket> <fichier.json>` |
| Lifecycle | `rc bucket lifecycle rule import <alias>/<bucket> <fichier.json>` |
| CORS | `rc bucket cors …` |
| Versioning | `rc bucket version …` |
| Infos cluster | `rc admin info cluster <alias>` |

Sortie JSON : option globale `--json` (ou `--format json`). L'app affiche ces commandes à
titre pédagogique ; elle ne les exécute pas.

## 6. Pièges constatés en production

1. **`rc bucket anonymous set-json` remplace toute la bucket policy.** Toute fusion doit
   être faite côté app avant l'appel.
2. **SSE-S3 casse tous les PUT** si `RUSTFS_SSE_S3_MASTER_KEY` n'est pas défini :
   `SSE-S3 requires RUSTFS_SSE_S3_MASTER_KEY to be set to a base64-encoded 32-byte key
   when KMS is not configured`. Ne pas activer le chiffrement depuis RustFS Config en v1.
3. **`PUT` vers un bucket non autorisé** peut renvoyer
   `Connection was closed before we received a valid response` au lieu d'un 403.
   Confirmer le refus par `ListObjects` / `HeadObject`.
4. **`readonly` n'inclut pas `ListBucket`** : le listing échoue, la lecture par clé
   connue fonctionne. Piège classique en support.
5. **`GetPublicAccessBlock` non supporté** :
   `NoSuchPublicAccessBlockConfiguration`. La protection contre l'exposition publique
   repose donc sur la bucket policy et sur les vérifications de RustFS Config.
6. **Un `AccessDenied` sur `DeleteObject` ne signifie pas bucket en lecture seule** : la
   policy de la clé peut n'autoriser que `Get`/`Put` (cas d'une clé MCP restreinte).
7. **Bucket policy anonyme** : `s3:GetObject` seul, jamais `ListBucket`, jamais
   `PutObject`. Un préfixe vide rend le bucket entier lisible.
8. **Test d'isolation** : le faire sur un bucket neuf et privé. Un bucket déjà public
   répond favorablement à un listing même sans droit IAM, ce qui fausse le test.
9. **`ListAllMyBuckets`** n'est pas accordé par une policy scopée : `list-buckets` doit
   être traité comme « non autorisé », pas comme une erreur de connexion.
10. **Lifecycle** : une réponse de `PutObject` porte un en-tête `Expiration` quand une
    règle s'applique — utile pour vérifier qu'une règle est active.
11. **`rc` 0.1.37** : commandes dépréciées encore présentes (`ls`, `mb`, `rm`, `cat`,
    `ilm`…) ; utiliser les formes `rc bucket …` / `rc object …`.
12. **Nom d'utilisateur tronqué** : `rc admin policy entities --user kwiktalk-a` renvoie
    `No matching policy entity mappings.` sans suggérer la faute de frappe. Toujours
    revérifier le nom exact avant de conclure à un problème d'attachement.

## 7. Ce qui a été validé manuellement

Sur une instance RustFS locale (bucket jetable) puis en production :

- création d'un user + policy scopée, `PUT`/`GET`/`DELETE` dans le bucket ;
- refus sur un bucket neuf privé (`AccessDenied`) et sur `ListAllMyBuckets` ;
- bucket policy publique par préfixe : `GET` anonyme 200, préfixe privé 403, listing
  anonyme 403 ;
- lifecycle : expiration appliquée, `AbortIncompleteMultipartUpload` accepté ;
- idempotence d'un second `apply` (aucun changement retourné par le diff).
