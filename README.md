# RustFS Config

Petite application desktop (Tauri 2 + Nuxt 4 + Nuxt UI 4) pour configurer des instances
RustFS sans retaper de JSON à la main : instances, buckets, policies IAM, accès public par
préfixe, lifecycle, puis génération des variables d'environnement à coller dans un projet
(Laravel en premier).

**Nom :** `RustFS Config` (définitif, 2026-11-07). Évite la collision avec le CLI `rc`.

Le problème d'origine : chaque nouveau projet refait le même travail — créer un bucket,
créer un user S3 limité à ce bucket, écrire la policy, autoriser la lecture anonyme de
`uploads/*`, poser un lifecycle, puis recopier huit variables `AWS_*` dans `.env`. Et le
plus souvent, la clé utilisée est une clé root non limitée : une fuite compromet toute
l'instance.

## État

v0.1.0 : instances (CRUD, test, qualification de clé), explorateur buckets/objets,
moteur IAM scopé (rotation, révocation, adoption), vérifications d'isolation, bucket
policy par préfixe avec diff, lifecycle/versioning/CORS, générateur `.env` (Laravel,
Node, AWS CLI) et parcours guidé. Voir [`SPEC.md`](./SPEC.md) pour le périmètre et
[`docs/RUSTFS-NOTES.md`](./docs/RUSTFS-NOTES.md) pour l'API admin, les policies et les
pièges relevés en conditions réelles.

## Prérequis

- Bun 1.4+ et Rust 1.82+ (toolchain Tauri).
- Dépendances système Tauri (webkit2gtk, etc.) : voir la doc Tauri.
- `rc` (CLI RustFS) **n'est pas requis** à l'exécution : l'app parle directement à l'API S3 et à
  l'API admin `/rustfs/admin/v3`. `rc` reste utile pour reproduire une opération en CLI ;
  l'UI affiche la commande équivalente (sans l'exécuter).
- Une instance RustFS joignable (locale ou distante) avec un compte autorisé aux actions
  `admin:*` pour la gestion IAM.

## Développement

```bash
bun i
bun run tauri:dev     # fenêtre Tauri + dev server Nuxt sur 127.0.0.1:3000
bun run tauri:build   # binaire de production
```

Scripts utiles : `bun run dev` (navigateur seul), `bun run generate` (build statique
consommé par Tauri via `frontendDist: ../dist`).

Tests backend sans interface (pas besoin de GTK/WebKit) :

```bash
cd src-tauri
cargo test --no-default-features
cargo clippy --all-targets --no-default-features -- -D warnings
```

Les tests d'intégration utilisent `RUSTFS_TEST_ENDPOINT`, `RUSTFS_TEST_ACCESS_KEY`,
`RUSTFS_TEST_SECRET_KEY` et `RC_TEST_KEYRING=1` ; ils sont ignorés/skippés sans ces
variables et ne touchent jamais une instance de production.

Suite complète contre l'image épinglée, comme en CI :

```bash
docker compose -f docker-compose.test.yml up -d
until curl -sf http://127.0.0.1:9200 -o /dev/null; do sleep 1; done
cd src-tauri
RUSTFS_TEST_ENDPOINT=http://127.0.0.1:9200 \
RUSTFS_TEST_ACCESS_KEY=rustfsadmin RUSTFS_TEST_SECRET_KEY=rustfsadmin \
RC_TEST_KEYRING=1 cargo test --no-default-features --all-targets
```

## Installation

Le workflow `.github/workflows/release.yml` construit les paquets pour Linux, macOS et
Windows à chaque tag `v*` et les attache à une release GitHub en brouillon.

| Plateforme | Paquet | Installation |
| --- | --- | --- |
| Linux | `.AppImage` / `.deb` / `.rpm` | `chmod +x RustFS-Config_*.AppImage` puis l'exécuter, ou `sudo dpkg -i` / `sudo rpm -i` |
| macOS | `.dmg` | Ouvrir le disque et glisser l'app dans *Applications* |
| Windows | `.msi` / `.exe` | Double-clic et suivre l'assistant |

L'app ne requiert ni `rc` ni variable d'environnement : elle parle directement à S3 et à
`/rustfs/admin/v3`. Les secrets sont dans le trousseau de l'OS, l'état non secret dans
`~/.config/rc/store.json` (Linux), `~/Library/Application Support/rc/store.json` (macOS)
ou `%APPDATA%\rc\store.json` (Windows). Le backend Rust est compilé dans le binaire.

Avant une distribution hors GitHub, remplacer les icônes de `src-tauri/icons/` et
configurer les clés de signature (`TAURI_SIGNING_PRIVATE_KEY` et
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`) pour l'updater.

## Désinstallation

| Plateforme | Commande |
| --- | --- |
| Linux `.deb` | `sudo apt remove rustfs-config` |
| Linux `.rpm` | `sudo rpm -e rustfs-config` |
| Linux AppImage | supprimer le fichier `.AppImage` |
| macOS | glisser l'app hors de *Applications* |
| Windows | *Paramètres → Applications → RustFS Config → Désinstaller* |

Supprimer ensuite les données locales si souhaité : le dossier `rc` cité ci-dessus et les
entrées du trousseau (`rustfs-config`, clés `rc://instance/<id>` et `rc://credential/<id>`).

## Structure

```
app/                  pages, composants, composables Nuxt (SSR désactivé)
i18n/locales/         chaînes FR / EN
src-tauri/            backend Rust : commandes Tauri, client S3, client admin, stockage
docs/                 notes techniques
SPEC.md               spécification fonctionnelle et technique
```

Le backend Rust est la seule couche qui voit les secrets ; la webview ne reçoit que des
identifiants masqués et des résultats. Voir les décisions dans `SPEC.md`.
