# RC — RustFS Config

Petite application desktop (Tauri 2 + Nuxt 4 + Nuxt UI 4) pour configurer des instances
RustFS sans retaper de JSON à la main : instances, buckets, policies IAM, accès public par
préfixe, lifecycle, puis génération des variables d'environnement à coller dans un projet
(Laravel en premier).

Le problème d'origine : chaque nouveau projet refait le même travail — créer un bucket,
créer un user S3 limité à ce bucket, écrire la policy, autoriser la lecture anonyme de
`uploads/*`, poser un lifecycle, puis recopier huit variables `AWS_*` dans `.env`. Et le
plus souvent, la clé utilisée est une clé root non limitée : une fuite compromet toute
l'instance.

## État

Phase de spécification. Aucune fonctionnalité n'est implémentée (le dépôt contient le
starter Nuxt UI / Tauri). Voir [`SPEC.md`](./SPEC.md) pour le périmètre et
[`docs/RUSTFS-NOTES.md`](./docs/RUSTFS-NOTES.md) pour l'API admin, les policies et les
pièges relevés en conditions réelles.

## Prérequis

- Bun 1.4+ et Rust 1.78+ (toolchain Tauri).
- Dépendances système Tauri (webkit2gtk, etc.) : voir la doc Tauri.
- `rc` **n'est pas requis** à l'exécution : l'app parle directement à l'API S3 et à
  l'API admin `/rustfs/admin/v3`. `rc` reste utile pour reproduire une opération en CLI ;
  l'UI affiche la commande équivalente.
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
