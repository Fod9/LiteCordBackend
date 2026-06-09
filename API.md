# LiteCord API Reference

## Conventions

- **Auth** : toutes les routes marquées 🔒 requièrent le header `Authorization: Bearer <token>`
- **Content-Type** : `application/json` pour tous les corps de requête
- **Erreurs** : corps texte brut, status HTTP correspondant à l'erreur
- **IDs** : format string `"<table>:<id>"`, ex: `"user:abc123"`, `"friendship:xyz789"`

---

## Auth — `/auth`

### `POST /auth/signup`

Crée un compte utilisateur.

**Corps**
```json
{
  "name": "string",
  "email": "string",
  "password": "string"
}
```

**Retour** `201`
```json
{
  "token": "string",
  "refresh_token": "string"
}
```

---

### `POST /auth/login`

Authentifie un utilisateur existant.

**Corps**
```json
{
  "email": "string",
  "password": "string"
}
```

**Retour** `200`
```json
{
  "token": "string",
  "refresh_token": "string"
}
```

---

### `POST /auth/refresh`

Échange un refresh token contre une nouvelle paire de tokens.

**Corps**
```json
{
  "refresh_token": "string"
}
```

**Retour** `200`
```json
{
  "token": "string",
  "refresh_token": "string"
}
```

---

### `GET /auth/me` 🔒

Retourne les informations publiques de l'utilisateur connecté.

**Retour** `200`
```json
{
  "id": "user:<id>",
  "name": "string",
  "display_name": "string",
  "profile_picture": "string"
}
```

---

## CDN — `/cdn`

### `POST /cdn/presign` 🔒

Génère une URL d'upload signée (valable 5 min). Le client upload ensuite **directement** vers RustFS via un PUT sur `upload_url`, sans passer par le backend.

**Corps**
```json
{
  "filename": "photo.jpg",
  "content_type": "image/jpeg",
  "size": 98765
}
```

**Retour** `200`
```json
{
  "upload_url": "https://s3.endpoint/bucket/uuid/photo.jpg?X-Amz-Signature=...",
  "cdn_url": "https://cdn.tondomaine.com/uuid/photo.jpg"
}
```

**Erreurs**
- `400` — `filename` ou `content_type` absent
- `413` — fichier dépasse 25 MB

**Flow complet**
```
POST /cdn/presign  →  { upload_url, cdn_url }
PUT  upload_url    →  200 (upload direct vers RustFS)
WS / POST message  →  { content: "...", attachments: [{ url: cdn_url, filename, size }] }
```

---

## Channels — `/channels`

### `POST /channels/dm` 🔒

Crée un DM channel (1-to-1 ou groupe). Idempotent : retourne le channel existant si les mêmes participants ont déjà un channel.

**Corps**
```json
{
  "recipient_ids": ["user:<id>"]
}
```

Le créateur (owner) est automatiquement ajouté aux participants.

**Retour** `201`
```json
{
  "id": "DMChannel:<id>",
  "name": "string | null",
  "owner": "user:<id>",
  "participants": [
    { "id": "user:<id>", "name": "string", "display_name": "string", "profile_picture": "string" }
  ],
  "recipients_key": "string",
  "last_message_id": "message:<id> | null",
  "created_at": "datetime"
}
```

**Erreurs**
- `400` — ID de destinataire invalide
- `403` — l'appelant n'est pas ami (statut `accepted`) avec tous les destinataires
- `404` — un ou plusieurs destinataires introuvables

---

### `GET /channels/list_dm` 🔒

Retourne les DM channels de l'utilisateur connecté et ses amitiés acceptées.

**Retour** `200` — tableau de deux tableaux : `[dmChannels, friendships]`
```json
[
  [
    {
      "id": "DMChannel:<id>",
      "name": "string | null",
      "owner": "user:<id>",
      "participants": [
        { "id": "user:<id>", "name": "string", "display_name": "string", "profile_picture": "string" }
      ],
      "recipients_key": "string",
      "last_message_id": "message:<id> | null",
      "created_at": "datetime"
    }
  ],
  [
    {
      "id": "friendship:<id>",
      "in": "user:<id>",
      "out": "user:<id>",
      "status": "accepted",
      "created_at": "datetime"
    }
  ]
]
```

---

### `GET /channels/<channel_id>/messages?limit=<n>&before=<message_id>` 🔒

Retourne l'historique de messages d'un channel (DMChannel ou channel de serveur), en ordre chronologique.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `channel_id` | string | ID du channel (`DMChannel:<id>` ou `channel:<id>`) |

**Query params**
| Paramètre | Type | Défaut | Description |
|---|---|---|---|
| `limit` | int | `50` | Nombre max de messages à retourner (max 100) |
| `before` | string | — | ID d'un message — retourne uniquement les messages plus anciens |

**Retour** `200`
```json
[
  {
    "id": "message:<id>",
    "channel": "DMChannel:<id> | channel:<id>",
    "author": {
      "id": "user:<id>",
      "name": "string",
      "display_name": "string",
      "profile_picture": "string"
    },
    "content": "string",
    "reply_to": "message:<id> | null",
    "attachments": [
      {
        "url": "string",
        "filename": "string",
        "size": 1234
      }
    ],
    "edited_at": "datetime | null",
    "created_at": "datetime"
  }
]
```

---

## Friends — `/friends`

### `POST /friends/add_friend/<friend_name>` 🔒

Envoie une demande d'amitié à un utilisateur par son nom.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `friend_name` | string | Nom exact de l'utilisateur cible |

**Retour** `200` — texte de confirmation

---

### `POST /friends/update_friend_request/<friendship_id>/<action>` 🔒

Accepte ou rejette une demande d'amitié.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `friendship_id` | string | ID du record friendship (`friendship:<id>`) |
| `action` | string | `accept` ou `reject` |

**Retour** `200` — texte de confirmation

---

### `DELETE /friends/<friendship_id>` 🔒

Supprime une amitié (fonctionne quel que soit le statut — pending ou accepted). Les deux parties de l'amitié peuvent déclencher la suppression.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `friendship_id` | string | `friendship:<id>` |

**Retour** `204`

**Notification WS** envoyée à l'autre partie :
```json
{ "message_type": "friend_removed", "content": "friendship:<id>" }
```

---

### `GET /friends/pending` 🔒

Liste les demandes d'amitié reçues et en attente (statut `pending`, dont l'utilisateur est destinataire).

**Retour** `200`
```json
[
  {
    "id": "friendship:<id>",
    "in_user": { "id": "user:<id>", "name": "string", "display_name": "string", "profile_picture": "string" },
    "out_user": { "id": "user:<id>", "name": "string", "display_name": "string", "profile_picture": "string" },
    "status": "pending",
    "created_at": "datetime"
  }
]
```

`in_user` est l'expéditeur, `out_user` est le destinataire (l'utilisateur connecté).

---

### `POST /friends/list_friends` 🔒

Liste les amis acceptés de l'utilisateur connecté.

**Retour** `200`
```json
[
  {
    "id": "friendship:<id>",
    "in_user": { "id": "user:<id>", "name": "string", "display_name": "string", "profile_picture": "string" },
    "out_user": { "id": "user:<id>", "name": "string", "display_name": "string", "profile_picture": "string" },
    "status": "accepted",
    "created_at": "datetime"
  }
]
```

---

## Guilds — `/guilds`

### `POST /guilds/` 🔒

Crée un serveur. L'utilisateur connecté en devient propriétaire et membre automatiquement.

**Corps**
```json
{
  "name": "string",
  "icon": "string"
}
```

**Retour** `201`
```json
{
  "id": "guild:<id>",
  "name": "string",
  "icon": "string",
  "owner": "user:<id>",
  "created_at": "datetime"
}
```

---

### `GET /guilds/` 🔒

Liste tous les serveurs dont l'utilisateur est membre.

**Retour** `200`
```json
[
  {
    "id": "guild:<id>",
    "name": "string",
    "icon": "string",
    "owner": "user:<id>",
    "created_at": "datetime"
  }
]
```

---

### `DELETE /guilds/<guild_id>` 🔒

Supprime un serveur. Réservé au propriétaire. Supprime membres et invitations.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |

**Retour** `204`

**Notification WS** envoyée à tous les membres du serveur (y compris le propriétaire) :
```json
{ "message_type": "guild_deleted", "content": "guild:<id>" }
```

---

### `POST /guilds/<guild_id>/invites` 🔒

Génère un code d'invitation. Réservé aux membres.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |

**Retour** `201`
```json
{
  "id": "guild_invite:<id>",
  "guild": "guild:<id>",
  "inviter": "user:<id>",
  "code": "string",
  "expires_at": "datetime | null",
  "created_at": "datetime"
}
```

---

### `POST /guilds/join/<code>` 🔒

Rejoint un serveur via un code d'invitation.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `code` | string | Code d'invitation (8 caractères alphanumériques) |

**Retour** `200`
```json
{
  "id": "guild:<id>",
  "name": "string",
  "icon": "string",
  "owner": "user:<id>",
  "created_at": "datetime"
}
```

**Notification WS** envoyée à tous les membres du serveur :
```json
{
  "message_type": "guild_member_joined",
  "content": "{\"guild_id\": \"guild:<id>\", \"user\": { ...SimpleUser }}"
}
```

---

### `PATCH /guilds/<guild_id>` 🔒

Met à jour le nom et/ou l'icône d'un serveur. Réservé au propriétaire. Les champs absents ou `null` sont conservés.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |

**Corps**
```json
{
  "name": "string | null",
  "icon": "string | null"
}
```

**Retour** `200`
```json
{
  "id": "guild:<id>",
  "name": "string",
  "icon": "string",
  "owner": "user:<id>",
  "created_at": "datetime"
}
```

**Erreurs**
- `403` — l'utilisateur n'est pas propriétaire
- `404` — serveur introuvable

---

### `GET /guilds/<guild_id>/members` 🔒

Liste les membres d'un serveur avec leur profil et leurs rôles. Réservé aux membres.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |

**Retour** `200`
```json
[
  {
    "id": "member_of:<id>",
    "user": {
      "id": "user:<id>",
      "name": "string",
      "display_name": "string",
      "profile_picture": "string"
    },
    "roles": ["role:<id>"],
    "nickname": "string | null",
    "joined_at": "datetime"
  }
]
```

**Erreurs**
- `403` — l'utilisateur n'est pas membre

---

### `POST /guilds/<guild_id>/members/<user_id>/kick` 🔒

Expulse un membre du serveur. Réservé au propriétaire.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `user_id` | string | `user:<id>` de la cible |

**Retour** `204`

**Erreurs**
- `400` — tentative d'expulser le propriétaire
- `403` — l'appelant n'est pas propriétaire
- `404` — membre introuvable

**Notification WS** envoyée à tous les membres restants :
```json
{
  "message_type": "guild_member_left",
  "content": "{\"guild_id\": \"guild:<id>\", \"user_id\": \"user:<id>\"}"
}
```

---

### `GET /guilds/<guild_id>/invites` 🔒

Liste les invitations actives d'un serveur. Réservé au propriétaire.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |

**Retour** `200`
```json
[
  {
    "id": "guild_invite:<id>",
    "guild": "guild:<id>",
    "inviter": "user:<id>",
    "code": "string",
    "expires_at": "datetime | null",
    "created_at": "datetime"
  }
]
```

---

### `DELETE /guilds/<guild_id>/invites/<invite_id>` 🔒

Révoque une invitation. Réservé au propriétaire.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `invite_id` | string | `guild_invite:<id>` |

**Retour** `204`

**Erreurs**
- `403` — l'appelant n'est pas propriétaire
- `404` — invitation introuvable ou n'appartient pas à ce serveur

---

### `POST /guilds/<guild_id>/leave` 🔒

Quitte un serveur. Le propriétaire ne peut pas quitter.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |

**Retour** `204`

**Notification WS** envoyée aux membres restants :
```json
{
  "message_type": "guild_member_left",
  "content": "{\"guild_id\": \"guild:<id>\", \"user_id\": \"user:<id>\"}"
}
```

---

## Guild Channels — `/guilds`

### `POST /guilds/<guild_id>/channels` 🔒

Crée un channel dans un serveur. Réservé aux membres.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |

**Corps**
```json
{
  "name": "string",
  "channel_type": "Text | Voice",
  "category": "string | null"
}
```

**Retour** `201`
```json
{
  "id": "channel:<id>",
  "guild": "guild:<id>",
  "name": "string",
  "channel_type": "Text | Voice",
  "category": "string | null",
  "created_at": "datetime"
}
```

**Notification WS** envoyée à tous les membres du serveur :
```json
{
  "message_type": "channel_created",
  "content": "{ ...Channel }"
}
```

---

### `GET /guilds/<guild_id>/channels` 🔒

Liste les channels d'un serveur. Réservé aux membres.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |

**Retour** `200`
```json
[
  {
    "id": "channel:<id>",
    "guild": "guild:<id>",
    "name": "string",
    "channel_type": "Text | Voice",
    "category": "string | null",
    "created_at": "datetime"
  }
]
```

---

### `DELETE /guilds/<guild_id>/channels/<channel_id>` 🔒

Supprime un channel et tous ses messages. Réservé au propriétaire du serveur.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `channel_id` | string | `channel:<id>` |

**Retour** `204`

**Notification WS** envoyée à tous les membres du serveur :
```json
{
  "message_type": "channel_deleted",
  "content": "{\"guild_id\": \"guild:<id>\", \"channel_id\": \"channel:<id>\"}"
}
```

---

## Roles — `/guilds`

### `POST /guilds/<guild_id>/roles` 🔒

Crée un rôle dans un serveur. Réservé au propriétaire.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |

**Corps**
```json
{
  "name": "string",
  "color": "string",
  "position": 1,
  "permissions": ["string"]
}
```

**Retour** `201`
```json
{
  "id": "role:<id>",
  "guild": "guild:<id>",
  "name": "string",
  "color": "string",
  "position": 1,
  "permissions": ["string"]
}
```

---

### `GET /guilds/<guild_id>/roles` 🔒

Liste les rôles d'un serveur, triés par position croissante.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |

**Retour** `200`
```json
[
  {
    "id": "role:<id>",
    "guild": "guild:<id>",
    "name": "string",
    "color": "string",
    "position": 1,
    "permissions": ["string"]
  }
]
```

---

### `DELETE /guilds/<guild_id>/roles/<role_id>` 🔒

Supprime un rôle et le retire de tous les membres. Réservé au propriétaire.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `role_id` | string | `role:<id>` |

**Retour** `204`

---

### `POST /guilds/<guild_id>/members/<user_id>/roles/<role_id>` 🔒

Assigne un rôle à un membre. Réservé au propriétaire.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `user_id` | string | `user:<id>` de l'utilisateur cible |
| `role_id` | string | `role:<id>` |

**Retour** `204`

**Notification WS** envoyée à tous les membres du serveur :
```json
{
  "message_type": "role_updated",
  "content": "{\"guild_id\": \"guild:<id>\", \"user_id\": \"user:<id>\", \"role_id\": \"role:<id>\", \"action\": \"assigned\"}"
}
```

---

### `DELETE /guilds/<guild_id>/members/<user_id>/roles/<role_id>` 🔒

Retire un rôle d'un membre. Réservé au propriétaire.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `user_id` | string | `user:<id>` de l'utilisateur cible |
| `role_id` | string | `role:<id>` |

**Retour** `204`

**Notification WS** envoyée à tous les membres du serveur :
```json
{
  "message_type": "role_updated",
  "content": "{\"guild_id\": \"guild:<id>\", \"user_id\": \"user:<id>\", \"role_id\": \"role:<id>\", \"action\": \"removed\"}"
}
```

---

## WebSocket — `/ws`

### `GET /ws/?token=<token>`

Ouvre une connexion WebSocket pour la messagerie temps réel.

| Query param | Description |
|---|---|
| `token` | *(optionnel)* Access token JWT — auth immédiate à la connexion |

---

### Authentification

Deux méthodes au choix :

**1. Query param (recommandé pour les outils de test)**

Passer le token dans l'URL : `ws://host/ws/?token=<token>`  
Le serveur répond immédiatement avec `{ "status": "authenticated" }`.

**2. Premier message WS**

Connexion sans query param, puis envoyer en premier message :

**Client → Serveur**
```json
{ "token": "string" }
```

**Serveur → Client** (succès)
```json
{
  "status": "authenticated",
  "friends_online": ["user:<id>"]
}
```

`friends_online` : liste des IDs d'amis déjà connectés au moment de l'auth. Permet d'initialiser l'état de présence sans polling.

**Serveur → Client** (token invalide)
```json
{ "error": "invalid token" }
```

---

### Envoi d'un message

**Client → Serveur**
```json
{
  "to": "string",
  "message_type": "string",
  "content": "string"
}
```

| Champ | Description |
|---|---|
| `to` | Cible : `user:<id>`, `DMChannel:<id>`, ou `channel:<id>` |
| `message_type` | Type libre (ex: `"text"`) |
| `content` | Contenu du message |
| `attachments` | *(optionnel)* Tableau de fichiers déjà uploadés via `/cdn/presign` |

**Format d'un attachment**
```json
{ "url": "string", "filename": "string", "size": 12345 }
```

| Valeur de `to` | Comportement |
|---|---|
| `user:<id>` | DM direct — crée le DMChannel si inexistant. Requiert une amitié `accepted` avec la cible. |
| `DMChannel:<id>` | Message dans un DM channel existant. Requiert une amitié `accepted` avec tous les autres participants. |
| `channel:<id>` | Message dans un channel de serveur |

---

### Réception d'un message

**Serveur → Client**
```json
{
  "message_type": "new_message",
  "content": "string"
}
```

`content` est la sérialisation JSON du message persisté, avec le profil auteur embarqué :
```json
{
  "id": "message:<id>",
  "channel": "DMChannel:<id> | channel:<id>",
  "author": {
    "id": "user:<id>",
    "name": "string",
    "display_name": "string",
    "profile_picture": "string"
  },
  "content": "string",
  "reply_to": "message:<id> | null",
  "attachments": [],
  "edited_at": "datetime | null",
  "created_at": "datetime"
}
```

---

### Événements serveur

Ces messages sont envoyés spontanément par le serveur suite à des actions d'autres utilisateurs.

**Ami connecté** — envoyé à tous les amis connectés quand un utilisateur s'authentifie sur le WS
```json
{ "message_type": "user_online", "user_id": "user:<id>" }
```

**Ami déconnecté** — envoyé à tous les amis connectés quand un utilisateur ferme sa connexion WS
```json
{ "message_type": "user_offline", "user_id": "user:<id>" }
```

---

**Autres événements** — suite à des actions HTTP

**Demande d'ami reçue** — envoyé au destinataire quand quelqu'un lui envoie une demande
```json
{
  "message_type": "friend_request",
  "content": "{\"friendship\": { ...Friendship }, \"from_user\": { ...SimpleUser }}"
}
```

`from_user` : `{ "id": "user:<id>", "name": "string", "display_name": "string", "profile_picture": "string" }`

**Demande d'ami mise à jour** — envoyé à l'expéditeur quand le destinataire accepte ou refuse
```json
{
  "message_type": "friend_request_updated",
  "content": "{\"friendship\": { ...Friendship }, \"from_user\": { ...SimpleUser }}"
}
```

`from_user` : l'utilisateur qui a accepté ou refusé la demande.

**DM channel créé** — envoyé au destinataire lors du premier message DM
```json
{
  "message_type": "dm_channel_created",
  "content": "DMChannel:<id>"
}
```

**Erreur d'envoi** — envoyé à l'expéditeur quand un message WS est rejeté (ex : amitié manquante)
```json
{
  "message_type": "error",
  "content": "string"
}
```

---

**Événements serveur (guild)** — diffusés à tous les membres connectés du serveur concerné

**Nouveau membre** — quand un utilisateur rejoint via `POST /guilds/join/<code>`
```json
{
  "message_type": "guild_member_joined",
  "content": "{\"guild_id\": \"guild:<id>\", \"user\": { \"id\": \"user:<id>\", \"name\": \"string\", \"display_name\": \"string\", \"profile_picture\": \"string\" }}"
}
```

**Membre parti/expulsé** — quand un membre quitte (`POST /leave`) ou est expulsé (`POST /kick`)
```json
{
  "message_type": "guild_member_left",
  "content": "{\"guild_id\": \"guild:<id>\", \"user_id\": \"user:<id>\"}"
}
```

**Channel créé** — quand un channel est ajouté au serveur
```json
{
  "message_type": "channel_created",
  "content": "{ ...Channel }"
}
```

**Channel supprimé**
```json
{
  "message_type": "channel_deleted",
  "content": "{\"guild_id\": \"guild:<id>\", \"channel_id\": \"channel:<id>\"}"
}
```

**Rôle modifié** — quand un rôle est assigné ou retiré à un membre
```json
{
  "message_type": "role_updated",
  "content": "{\"guild_id\": \"guild:<id>\", \"user_id\": \"user:<id>\", \"role_id\": \"role:<id>\", \"action\": \"assigned | removed\"}"
}
```

**Serveur supprimé** — envoyé à tous les membres connectés quand le propriétaire supprime le serveur via `DELETE /guilds/<guild_id>`
```json
{ "message_type": "guild_deleted", "content": "guild:<id>" }
```

---

### Refresh de token

Le serveur vérifie le token toutes les 300 secondes. S'il a expiré :

**Serveur → Client**
```json
{ "action": "token_refresh_required" }
```

Le client a 30 secondes pour répondre avec un refresh token valide :

**Client → Serveur**
```json
{ "refresh_token": "string" }
```

**Serveur → Client** (succès)
```json
{
  "status": "token_refreshed",
  "token": "string",
  "refresh_token": "string"
}
```

**Serveur → Client** (timeout)
```json
{ "error": "token_refresh_timeout" }
```
La connexion est fermée immédiatement après.
