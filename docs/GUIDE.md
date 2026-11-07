# Guide utilisateur — RustFS Config

Ce guide mène de l'installation au premier bucket prêt pour une application Laravel, sans
écrire de JSON et sans créer de clé root.

## 1. Installer

Télécharger le paquet de la plateforme depuis la page des releases, puis :

| Plateforme | Paquet | Commande |
| --- | --- | --- |
| Linux | AppImage | `chmod +x RustFS-Config_*.AppImage` puis double-clic |
| Linux | `.deb` | `sudo dpkg -i rustfs-config_*.deb` |
| Linux | `.rpm` | `sudo rpm -i rustfs-config_*.rpm` |
| macOS | `.dmg` | glisser l'app dans *Applications* |
| Windows | `.msi` | double-clic et suivre l'assistant |

L'application ne demande ni `rc` ni variable d'environnement. Les secrets sont conservés
par le trousseau du système (GNOME Keyring, KWallet, Keychain ou Credential Manager) ;
l'état non secret vit dans `~/.config/rc/store.json` (Linux),
`~/Library/Application Support/rc/store.json` (macOS) ou `%APPDATA%\rc\store.json`
(Windows).

## 2. Ajouter une instance

1. Cliquer sur **Ajouter une instance** dans la barre latérale.
2. Renseigner le libellé, l'endpoint (`http://127.0.0.1:9000` pour une instance locale),
   la région (`us-east-1` convient à RustFS), et laisser le path-style activé.
3. Saisir l'access key et le secret d'un compte autorisé aux actions `admin:*`.
4. Cliquer sur **Tester**. L'app affiche le nombre de buckets et le niveau de la clé :
   - **root** : la clé est celle du serveur ; l'app la signale comme dangereuse pour un
     usage applicatif ;
   - **admin** : `consoleAdmin` ou policy contenant `admin:*` ;
   - **clé applicative** : les écrans IAM sont désactivés.
5. Enregistrer. Le secret part au trousseau ; le store ne garde que les quatre derniers
   caractères.

## 3. Premier bucket (parcours KwikTalk)

Le bouton **Préparer un bucket** ouvre l'assistant :

1. **Bucket** — choisir le template *Laravel media*, ou *Vide* pour partir de zéro.
   Créer `kwiktalk` ou sélectionner un bucket existant.
2. **Visibilité** — garder `uploads/media/` (lecture anonyme des médias) et ajouter
   d'autres préfixes publics si besoin. Tout le reste demeure privé.
3. **Credentials** — mode `rw`, access key `kwiktalk-app` (valeur par défaut).
4. **Rétention** — expiration à 180 jours sur `uploads/media/`. La purge des uploads
   multipart incomplets (7 jours) est ajoutée automatiquement.
5. **Aperçu** — vérifier le diff de policy et de lifecycle, et les commandes `rc`
   équivalentes. Cliquer sur **Appliquer**.

Résultat : bucket créé, lecture publique limitée au préfixe, user IAM scopé, lifecycle
posé, et bloc `.env` affiché. Le même parcours est disponible onglet par onglet pour
modifier une configuration existante.

## 4. Utiliser le `.env`

Ouvrir l'onglet **.env**, choisir le credential et le format :

- **Copier** place le bloc dans le presse-papiers ;
- **Enregistrer sous…** écrit un fichier en permissions `0600` (confirmation si le
  fichier existe déjà).

Pour Laravel, coller le bloc dans `.env`, puis vérifier qu'un upload atterrit bien dans le
bucket et que son URL répond sans authentification lorsqu'il est sous `uploads/media/`.

## 5. Faire tourner une clé

Onglet **Accès**, bouton de rotation sur le credential. L'app génère un nouveau secret
côté RustFS (même access key), met à jour le trousseau et affiche le secret une seule fois.
Mettre à jour le `.env` de l'application : l'ancien secret est refusé immédiatement.

Pour révoquer : bouton de révocation, puis saisir l'access key exacte. Le user est
désactivé, détaché et supprimé ; le bucket et ses objets ne sont pas touchés.

## 6. Dépannage

| Symptôme | Cause probable | Solution |
| --- | --- | --- |
| `Signature is required` | requête admin non signée (hors app) | utiliser l'app ou signer en SigV4 service `s3` |
| `missing header: x-amz-content-sha256` | client SigV4 incomplet | utiliser RustFS Config à jour |
| Connexion refusée | endpoint ou TLS invalide | vérifier l'URL ; pour un hôte local uniquement, désactiver la vérification TLS |
| `AccessDenied` sur `DeleteObject` | clé réelle trop limitée | vérifier les droits effectifs, ne pas conclure que le bucket est en lecture seule |
| Bucket « vide » mais suppression refusée | objets ou versions restants | vider le bucket (et ses versions) avant de le supprimer |
| Un `PUT` échoue avec une connexion fermée | bucket non autorisé | c'est un refus d'accès déguisé ; confirmer avec `ListObjects` |
| `SSE-S3 requires RUSTFS_SSE_S3_MASTER_KEY` | chiffrement activé sans clé maître | ne pas activer SSE-S3 depuis l'app en v1 |
| Aucun secret trouvé (`keyring`) | trousseau verrouillé | déverrouiller GNOME Keyring / KWallet, puis rouvrir l'instance |
| Lecture anonyme renvoie 403 | bucket policy absente ou préfixe non déclaré | relancer l'aperçu de l'onglet Policy et appliquer |

## 7. Désinstaller

Désinstaller le paquet (voir `README.md`), puis supprimer si souhaité le dossier `rc` cité
au §1 et les entrées du trousseau `rustfs-config` (clés `rc://instance/<id>` et
`rc://credential/<id>`). Le bucket distant et ses objets ne sont jamais supprimés par la
désinstallation.
