# Changelog

Toutes les évolutions notables de RustFS Config. Format : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/), versions [SemVer](https://semver.org/lang/fr/).

## [0.1.2] — 2026-11-11

Aucun changement de code applicatif : cette version sert à vérifier la chaîne de mise à jour
depuis la 0.1.1 installée.

### Corrigé

- CI : la sonde de démarrage de RustFS exigeait une réponse 2xx alors que l'API répond `403`
  sur `/`, donc la suite d'intégration ne s'exécutait jamais sur GitHub Actions. La sonde
  accepte désormais toute réponse HTTP, `libdbus-1-dev` est installé et le trousseau est
  déverrouillé (`gnome-keyring-daemon --unlock`) avant les tests.

## [0.1.1] — 2026-11-11

### Ajouté

- Rapports d'erreur Sentry pour le frontend (`@sentry/vue`) et le backend Rust (`sentry` 0.42) :
  erreurs de commande, exceptions, panics et logs, sans donnée personnelle.
- Masquage des secrets avant envoi : clés AWS (`AKIA…`), paramètres de signature
  `x-amz-*`, en-têtes `Authorization` et champs `password`/`*_key` sont remplacés par `***`.
- Dépôt public : la mémoire projet locale et les outils d'agents propres à la machine ne sont
  plus suivis par git.

## [0.1.0] — 2026-11-07


### Ajouté

- Instances RustFS : ajout, édition, suppression, test de connexion et qualification de clé (root, admin, applicative, invalide).
- Explorateur buckets/objets : listing par préfixe, création/suppression de bucket, upload, téléchargement, suppression d'objet.
- Moteur IAM : users scopés à un bucket (`rw`, `ro`, `rw-sans-suppression`), rotation de secret, révocation, adoption d'une clé existante.
- Vérifications d'isolation après application (bucket attendu, sonde PUT/DELETE, lecture anonyme publique/privée, listing anonyme, bucket de contrôle).
- Bucket policy : lecture publique par préfixe, fusion par `Sid`, diff avant application.
- Lifecycle, versioning et CORS avec aperçu, sonde d'expiration et confirmation d'écrasement CORS.
- Générateur `.env` Laravel, Node (`@aws-sdk/client-s3`) et AWS CLI, copie et écriture en 0600.
- Parcours guidé « bucket prêt pour mon application » en quatre étapes.
- Interface FR/EN, thème Nuxt UI, CSP stricte, permissions Tauri minimales.
- CI de release Linux/macOS/Windows et suite d'intégration contre une image RustFS épinglée.

### Sécurité

- Secrets stockés uniquement dans le trousseau OS ; `store.json` ne contient que des identifiants masqués.
- Aucune clé root par défaut ; un user scopé est créé pour chaque bucket.
