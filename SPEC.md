# SPEC — RustFS Config

Version 0.1 — 07/11/2026. Statut : **validée** — 07/11/2026.

**Nom définitif :** `RustFS Config` (définitif le 07/11/2026, évite la collision avec le CLI `rc`).

---

## 1. Objectif

Permettre à un développeur de préparer un bucket RustFS pour une application, en moins de
cinq minutes, sans écrire de JSON et sans créer de clé non limitée.

Trois résultats concrets :

1. un bucket configuré (visibilité publique par préfixe, lifecycle, versioning, CORS) ;
2. un utilisateur IAM dédié, dont la policy ne donne accès qu'à ce bucket ;
3. un bloc de variables d'environnement prêt à coller dans le projet (Laravel, Node, CLI).

L'app gère plusieurs instances RustFS (locale, distante) et plusieurs buckets par instance.

## 2. Utilisateur cible

Développeur solo ou petite équipe qui auto-héberge RustFS et déploie des applications
Laravel / Node. Il connaît S3 mais pas l'API admin RustFS. Il veut un résultat vérifiable,
pas un tableau de bord de cluster.

## 3. Périmètre

### v1 (RustFS uniquement)

- Enregistrer / éditer / supprimer des instances (endpoint, région, path-style, TLS).
- Tester la connexion et qualifier la clé fournie (root, admin, limitée, invalide).
- Lister, créer, supprimer des buckets (suppression avec saisie du nom).
- Naviguer dans les objets : liste paginée, préfixes, upload, téléchargement, suppression.
- Policies IAM : créer un user scopé à un bucket, générer et attacher la policy,
  lister les users d'un bucket, désactiver, révoquer, faire tourner le secret.
- Bucket policy : lecture anonyme limitée à des préfixes déclarés, fusion idempotente,
  prévisualisation du diff avant application.
- Lifecycle : expiration par préfixe, purge des multipart incomplets.
- Versioning, CORS, quota (lecture + écriture simple).
- Générateur `.env` : Laravel, Node `@aws-sdk/client-s3`, AWS CLI ; copie, téléchargement,
  écriture dans un fichier (permissions 0600).
- Export / import de la configuration locale (sans secrets).

### Hors périmètre v1

- Autres fournisseurs S3 (AWS, MinIO, Scaleway…) — prévu v2, le modèle doit le permettre.
- Administration du cluster : nœuds, scaling, erasure coding, TLS, OIDC, audit targets.
- Gestion des groupes IAM et des service accounts (lecture seule éventuelle).
- Édition libre de policies JSON (v1 = templates + un mode avancé en lecture seule).
- Synchronisation cloud, comptes multi-utilisateurs, collaboration.
- Navigation de fichiers complète type Finder (v1 = liste + actions de base).

## 4. Décisions d'architecture

### 4.1 Backend natif Rust, pas de subprocess `rc`

`rc` est en 0.1.x, doit être présent dans le `PATH` d'une app packagée, et sa couverture
JSON n'est pas contractuelle. L'API admin `/rustfs/admin/v3` est documentée et signée
SigV4, donc reproductible en Rust.

- S3 : `aws-sdk-s3` + `aws-config` (`force_path_style`, `Region::new("us-east-1")`,
  credentials statiques).
- Admin : `reqwest` + `aws-sigv4` pour signer les requêtes `/rustfs/admin/v3/*`.
- Un trait `RustfsBackend` isole les deux implémentations ; un adaptateur CLI (`rc`) peut
  être ajouté plus tard sans toucher l'UI.

`rc` reste la référence de parité : chaque écran affiche la commande `rc` équivalente
(utile pour les scripts et le support).

### 4.2 Les secrets ne quittent jamais Rust

- Les clés (admin et générées) sont stockées dans le trousseau de l'OS via le crate
  `keyring` (Secret Service / Keychain / Credential Manager) sous
  `rc://instance/<id>` et `rc://credential/<id>`.
- Le frontend ne reçoit qu'un identifiant, un suffixe masqué (`AKIA…7F2Q`) et un booléen
  « secret disponible ».
- La signature des requêtes et la génération du `.env` se font côté Rust. Le secret n'est
  affiché qu'une fois, à la création, via une commande dédiée qui exige une confirmation.

### 4.3 Frontend Nuxt SPA, état minimal

SSR déjà désactivé. Pas de Pinia en v1 : deux composables (`useInstances`, `useBucket`)
suffisent. Nuxt UI pour les composants, `@nuxtjs/i18n` pour FR/EN, Lucide pour les icônes.

Toute opération passe par `invoke()` vers des commandes Tauri typées ; les erreurs Rust
sont normalisées en `{ code, message, hint }` pour être affichables telles quelles.

### 4.4 Plan puis application

Toute modification distante suit deux étapes :

1. `plan` — calcule l'état cible, le diff avec l'état lu, et les actions à effectuer ;
2. `apply` — exécute, renvoie un rapport par action (ok / skip / erreur).

Aucune écriture directe depuis un formulaire. Cela évite les remplacements involontaires
(voir §9 : `anonymous set-json` remplace toute la bucket policy) et rend l'app utilisable
sur un bucket existant.

### 4.5 Idempotence et propriété des ressources

L'app ajoute un marqueur aux ressources qu'elle crée : préfixe de policy `rc-`, tag
d'instance dans le nom du user (`app-<bucket>`), et un fichier local
`~/.config/rc/state.json` qui conserve la correspondance. Au premier contact avec un
bucket inconnu, l'app propose « adopter » : elle lit l'existant et ne supprime jamais une
ressource qu'elle n'a pas créée sans confirmation explicite.

## 5. Modèle de données local

`~/.config/rc/store.json` (non secret) :

```jsonc
{
  "version": 1,
  "instances": [
    {
      "id": "ins_01",
      "label": "Prod Aplix",
      "endpoint": "https://storage.applix.fr",
      "region": "us-east-1",
      "pathStyle": true,
      "tlsVerify": true,
      "consoleUrl": "https://console.applix.fr",
      "adminAccessKeySuffix": "…9XQ2",
      "lastCheck": { "at": "2026-11-07T09:00:00Z", "ok": true, "buckets": 4 }
    }
  ],
  "buckets": [
    {
      "instanceId": "ins_01",
      "name": "kwiktalk",
      "template": "laravel-media",
      "publicPrefixes": ["uploads/media/", "message-templates/"],
      "privatePrefixes": ["whatsapp-media/"],
      "lifecycle": [{ "prefix": "uploads/media/", "expireDays": 180 }],
      "managed": { "bucketPolicy": true, "lifecycle": true, "iam": true }
    }
  ],
  "credentials": [
    {
      "id": "cred_01",
      "instanceId": "ins_01",
      "bucket": "kwiktalk",
      "accessKey": "kwiktalk-app",
      "policyName": "rc-kwiktalk-rw",
      "mode": "rw",
      "createdAt": "2026-11-07T09:10:00Z",
      "rotatedAt": null
    }
  ]
}
```

Le secret correspondant est dans le trousseau, jamais dans ce fichier. Les ids sont
opaques et stables ; les noms de bucket restent la clé fonctionnelle côté RustFS.

## 6. Flux

### 6.1 Ajouter une instance

1. Formulaire : libellé, endpoint, région (défaut `us-east-1`), path-style (défaut oui),
   vérification TLS, access key, secret.
2. `Test` : `ListBuckets` + `GetBucketLocation` + un `user-info` sur la clé admin.
3. Qualification de la clé : root (access key = celle de l'instance), admin
   (`consoleAdmin` ou policy contenant `admin:*`), limitée (« clé applicative »).
   Une clé limitée est acceptée mais l'app désactive les écrans IAM et affiche la raison.
4. Le secret part au trousseau ; le store ne garde que le suffixe.

### 6.2 Préparer un bucket pour une application

Assistant en quatre étapes, chaque étape modifiable ensuite :

1. **Bucket** — nom, création si absent, versioning (défaut off).
2. **Visibilité** — préfixes lisibles anonymement (défaut aucun), préfixes privés
   (rappel visuel). Génère la bucket policy.
3. **Credentials** — mode `rw`, `ro`, `rw-sans-suppression` ; nom du user
   (`<bucket>-app` par défaut) ; policy générée scopée au bucket.
4. **Rétention** — expiration par préfixe, purge multipart (7 j par défaut).

Puis `Aperçu` (diff lisible + JSON repliable + commandes `rc` équivalentes) → `Appliquer`.
Le résultat affiche : user créé, policy attachée, vérifications automatiques passées
(voir §10), et le bloc `.env`.

### 6.3 Rotation et révocation

- Rotation : nouveau secret côté RustFS (`add-user` avec le même access key), mise à jour
  du trousseau, invalidation du `.env` précédemment copié (avertissement).
- Révocation : `set-user-status disabled`, détachement de la policy, puis suppression du
  user après confirmation. Le bucket et les objets ne sont jamais touchés.

### 6.4 Export `.env`

Sélection du credential + du format. Trois sorties :

- copier dans le presse-papiers ;
- télécharger un fichier `.env` (0600) ;
- écrire dans un chemin choisi, après confirmation si le fichier existe (jamais de
  remplacement silencieux).

## 7. Templates de policies

### 7.1 Policy IAM scopée (mode `rw`)

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "ListBucket",
      "Effect": "Allow",
      "Action": ["s3:ListBucket", "s3:GetBucketLocation"],
      "Resource": ["arn:aws:s3:::BUCKET"]
    },
    {
      "Sid": "ReadWriteObjects",
      "Effect": "Allow",
      "Action": [
        "s3:GetObject",
        "s3:PutObject",
        "s3:DeleteObject",
        "s3:AbortMultipartUpload",
        "s3:ListMultipartUploadParts"
      ],
      "Resource": ["arn:aws:s3:::BUCKET/*"]
    }
  ]
}
```

`ro` retire les actions d'écriture ; `rw-sans-suppression` ajoute un statement `Deny`
explicite sur `s3:DeleteObject` et `s3:DeleteObjectVersion` (un `Deny` gagne toujours).

### 7.2 Bucket policy (lecture anonyme par préfixe)

Statements générés uniquement pour les préfixes déclarés, un statement par préfixe :

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "PublicReadUploads",
      "Effect": "Allow",
      "Principal": { "AWS": ["*"] },
      "Action": ["s3:GetObject"],
      "Resource": ["arn:aws:s3:::BUCKET/uploads/media/*"]
    }
  ]
}
```

Règles :

- jamais de `s3:ListBucket` anonyme ;
- jamais de `s3:PutObject` anonyme ;
- si une bucket policy existe déjà, l'app fusionne par `Sid` et conserve les statements
  qu'elle ne gère pas ; le diff montre ce qui est ajouté, modifié, supprimé ;
- préfixe vide = bucket entier : interdit sans double confirmation.

### 7.3 Lifecycle

```json
{
  "Rules": [
    {
      "ID": "rc-expire-uploads-media",
      "Status": "Enabled",
      "Filter": { "Prefix": "uploads/media/" },
      "Expiration": { "Days": 180 }
    },
    {
      "ID": "rc-abort-incomplete-multipart",
      "Status": "Enabled",
      "Filter": { "Prefix": "" },
      "AbortIncompleteMultipartUpload": { "DaysAfterInitiation": 7 }
    }
  ]
}
```

Les règles portant l'identifiant `rc-` sont gérées par l'app, les autres sont conservées.
Après l'apply, une vérification fait un `PUT` sonde sous le préfixe : la présence de
l'en-tête `Expiration` prouve que la règle est active, et l'objet sonde est toujours
supprimé.

Versioning et CORS s'écrivent depuis le même plan. Une configuration CORS existante
posée par une autre source n'est écrasée qu'après saisie du nom du bucket.

## 8. Sorties `.env`

### Laravel (format par défaut)

```dotenv
FILESYSTEM_DISK=s3
AWS_ACCESS_KEY_ID=kwiktalk-app
AWS_SECRET_ACCESS_KEY=<secret>
AWS_DEFAULT_REGION=us-east-1
AWS_BUCKET=kwiktalk
AWS_ENDPOINT=https://storage.applix.fr
AWS_URL=https://storage.applix.fr/kwiktalk
AWS_USE_PATH_STYLE_ENDPOINT=true
AWS_SSL_VERIFY=true
```

`AWS_URL` suit le style path-style quand activé, sinon `https://<bucket>.<endpoint>`.
`AWS_SSL_VERIFY` n'est émis qu'en prod ; en local (endpoint non TLS) il est omis.

### Node (`@aws-sdk/client-s3`)

```dotenv
S3_ENDPOINT=https://storage.applix.fr
S3_REGION=us-east-1
S3_BUCKET=kwiktalk
S3_ACCESS_KEY_ID=kwiktalk-app
S3_SECRET_ACCESS_KEY=<secret>
S3_FORCE_PATH_STYLE=true
```

### AWS CLI

```bash
aws configure set aws_access_key_id kwiktalk-app --profile kwiktalk
aws configure set aws_secret_access_key <secret> --profile kwiktalk
aws configure set region us-east-1 --profile kwiktalk
aws --profile kwiktalk --endpoint-url https://storage.applix.fr s3 ls s3://kwiktalk
```

## 9. Sécurité

- **Moindre privilège par défaut** : l'assistant crée toujours un user scopé. Utiliser la
  clé admin de l'instance pour un projet est un choix explicite, précédé d'un
  avertissement bloquant (case à cocher).
- **Détection de clé dangereuse** : au moment de l'ajout d'une instance, si la clé
  retourne `ListAllMyBuckets` avec plus d'un bucket ou si sa policy contient `s3:*`,
  `readwrite` ou `consoleAdmin`, l'app l'indique et propose de créer une clé applicative.
- **Secrets** : trousseau OS uniquement ; aucun secret dans `store.json`, les logs, les
  rapports d'erreur ou les exports. Presse-papiers vidé après 60 s si l'option est active.
- **Transport** : TLS vérifié par défaut ; l'option « ignorer la vérification » est
  réservée aux instances locales et affichée en rouge.
- **CSP** : remplacer `csp: null` par une politique stricte (`default-src 'self'`).
- **Pas de télémétrie**, pas d'appel réseau hors de l'instance configurée.
- Les actions destructives (suppression de bucket, révocation de credential, suppression
  d'objet) demandent une confirmation nommée, jamais un simple « OK ».

## 10. Vérifications automatiques après application

Chaque `apply` se termine par des contrôles réels, affichés dans le rapport :

1. `ListBuckets` avec la clé générée → exactement le bucket attendu (ou refus si RustFS
   n'autorise pas `ListAllMyBuckets`).
2. `PutObject` d'une sonde `_rc-probe` → succès ; `DeleteObject` de la sonde → succès.
3. `GetObject` anonyme sur un préfixe public → 200 ; sur un préfixe privé → 403.
4. `ListObjects` anonyme → 403.
5. Accès à un autre bucket avec la clé générée → `AccessDenied` (test sur un bucket
   privé de contrôle, jamais sur un bucket de production inconnu).
6. Lecture de la bucket policy et du lifecycle → conformes au plan.

La sonde est supprimée dans tous les cas (succès ou échec) et son chemin est unique.

## 11. UI

Fenêtre 800×600 minimum, redimensionnable, une seule fenêtre.

```
┌ Sidebar ────────────┬ Contenu ──────────────────────────────────┐
│ RustFS Config       │ Prod Aplix · kwiktalk          [⋯]        │
│ ▾ Prod Aplix        │ Onglets : Vue d'ensemble · Objets ·       │
│   • kwiktalk  ●     │ Accès · Policy · Lifecycle · .env         │
│   • backups         │                                           │
│ ▸ Local (lerd)      │ [Préparer un bucket]  [Tester l'accès]    │
│ + Ajouter une       │                                           │
│   instance          │                                           │
└─────────────────────┴───────────────────────────────────────────┘
```

- État de connexion par instance (vert / orange / rouge) avec la dernière erreur lisible.
- Onglet « Accès » : deux cartes distinctes, **Utilisateur applicatif (IAM)** et
  **Lecture publique (bucket policy)** — la confusion entre les deux est la première
  source d'erreur.
- Toute policy affichée en JSON replié, avec la commande `rc` équivalente copiable.
- Pas de graphiques, pas de dashboard : l'information utile est un diff et un état.
- FR par défaut, EN complet.

## 12. Erreurs et cas limites

| Cas | Comportement |
| --- | --- |
| Endpoint injoignable / TLS invalide | Erreur de connexion avec certificat affiché ; proposer « ignorer TLS » pour un hôte local uniquement. |
| Clé sans droit admin | Écrans IAM désactivés, message « clé applicative — gestion IAM indisponible ». |
| `AccessDenied` sur `DeleteObject` | Cas réellement rencontré avec une clé MCP : afficher un avertissement sur les droits effectifs, ne pas conclure que le bucket est en lecture seule. |
| Bucket policy existante | Fusion par `Sid`, diff obligatoire, jamais de remplacement brut. |
| `PutObject` interrompu au lieu de 403 sur bucket non autorisé | RustFS peut fermer la connexion : mapper sur « accès refusé (connexion fermée) » et confirmer par `ListObjects`/`HeadObject`. |
| SSE-S3 activé sans clé maître | `SSE-S3 requires RUSTFS_SSE_S3_MASTER_KEY…` : ne jamais activer le chiffrement depuis l'app en v1 ; afficher l'explication. |
| Bucket non vide | Suppression refusée ; proposer le vidage explicite avec saisie du nom. |
| Policy référençant un user supprimé | Signaler l'orphelin dans l'onglet Accès. |
| Deux instances sur le même endpoint | Autorisé, mais avertir (risque de doublons de users). |

## 13. Critères d'acceptation v1

1. Sur une instance RustFS vierge, préparer un bucket « type KwikTalk » (lecture publique
   `uploads/media/`, lifecycle 180 j, user scopé) en moins de 5 minutes, sans CLI.
2. Le `.env` généré fonctionne tel quel dans un projet Laravel : upload public, URL
   accessible anonymement, fichier privé refusé (403).
3. La clé générée ne permet ni de lister, ni de lire, ni d'écrire dans un autre bucket.
4. Rejouer le même assistant sur le même bucket ne produit aucun changement (idempotence).
5. Une bucket policy préexistante non gérée par RustFS Config survit à l'application (fusion,
   pas remplacement).
6. Aucun secret en clair sur le disque, dans les logs ou dans la webview après création.
7. L'app démarre et fonctionne hors ligne (hors instance), sans binaire externe.

## 14. Jalons

| Jalon | Contenu | Cible |
| --- | --- | --- |
| M0 | Spécification validée (ce document) | 10/11/2026 |
| M1 | Socle : backend Rust, trousseau, store, coquille UI | 19/11/2026 |
| M2 | Instances + exploration buckets/objets | 26/11/2026 |
| M3 | Moteur IAM + vérification d'isolation | 02/12/2026 |
| M4 | Bucket policy, lifecycle, versioning, CORS | 08/12/2026 |
| M5 | Générateur `.env` + export | 10/12/2026 |
| M6 | Parcours guidé, packaging, signature | 19/12/2026 |
| QA | Tests d'intégration + documentation | 22/12/2026 |

## 15. Risques

- **API admin v3 non contractuelle** : peut changer entre versions de RustFS. Mitigation :
  tests d'intégration contre une image RustFS épinglée, adaptateur versionné, messages
  d'erreur explicites.
- **Signature SigV4 des routes admin** : à valider tôt (service `s3`, path-style) ; c'est
  le point technique le plus risqué du projet.
- **Secrets dans le trousseau Linux** : dépend de Secret Service (GNOME Keyring). Prévoir
  un fallback documenté (fichier chiffré avec phrase de passe) plutôt qu'un stockage plat.
- **Confusion IAM / bucket policy** : risque produit, traité par l'UI et la documentation
  intégrée.
- **Périmètre qui gonfle** : toute demande hors §3 part en v2.

## 16. Questions ouvertes — **répondues le 07/11/2026 (M0)**

1. **Nom définitif : `RustFS Config`.** Chose le 07/11/2026. `RC` collisionnait avec le CLI
   `rc` ; `RustFS Config` est clair, parlant, et ne dit pas « outil interne ».
   Le dépôt GitHub est `STIFLEUR390/rustfs-config`. Le binaire s'appellera `rustfs-config`.
   Les alternatives `Bucketwright` et `Seau` were not retained (moins parlantes).
2. **Embarquement de `rc` : affichage seulement.** Aucun bouton « exécuter cette commande »
   en v1. L'app affiche la commande `rc` équivalente (copiable), mais ne la lance pas :
   dépendance binaire non contractuelle, et exécuter un subprocess depuis une app desktop
   ajoute une surface d'attaque et un point de défaillance. Révision possible en v2 si la
   demande revient.
3. **Groupes IAM : v2.** Hors périmètre v1 (§3). Les service accounts et groupes sont
   prévus en v2, une fois le modèle de base (user + policy par bucket) stabilisé.
4. **Formats d'export supplémentaires : v2.** Docker Compose, Symfony, Rails sont
   reportés à v2. Le générateur v1 couvre Laravel (défaut), Node `@aws-sdk/client-s3`,
   et AWS CLI. Les retours terrain détermineront les prochains formats.
