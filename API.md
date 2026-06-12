# LiteCord API Reference

## Conventions

- **Auth** : toutes les routes marquées 🔒 requièrent le header `Authorization: Bearer <token>`
- **Content-Type** : `application/json` pour tous les corps de requête
- **Erreurs** : corps texte brut, status HTTP correspondant à l'erreur — **sauf** les erreurs de permission, qui ont un format JSON stable (voir ci-dessous)
- **IDs** : format string `"<table>:<id>"`, ex: `"user:abc123"`, `"friendship:xyz789"`

---

## Permissions

### Vocabulaire

Les permissions sont des identifiants snake_case stockés tels quels dans `role.permissions`. Toute valeur hors vocabulaire est refusée à l'écriture (`400`), ignorée au calcul si elle traîne en base, et purgée au prochain `PATCH` du rôle.

| Catégorie | ID | Effet |
|---|---|---|
| Serveur | `administrator` | Accorde **toutes** les permissions (bypass hiérarchie inclus) |
| Serveur | `manage_guild` | Modifier nom/icône du serveur |
| Serveur | `manage_roles` | CRUD des rôles + assignation/retrait aux membres |
| Serveur | `manage_channels` | Créer/supprimer des channels |
| Serveur | `create_invite` | Générer des codes d'invitation |
| Serveur | `manage_invites` | Lister et révoquer les invitations |
| Membres | `kick_members` | Expulser des membres |
| Membres | `ban_members` | *(réservé, endpoints à venir)* |
| Membres | `manage_nicknames` | *(réservé, endpoints à venir)* |
| Texte | `view_channels` | Voir les channels et lire l'historique |
| Texte | `send_messages` | Envoyer des messages (WS) |
| Texte | `attach_files` | Joindre des fichiers aux messages |
| Texte | `manage_messages` | *(réservé, endpoints à venir)* |
| Texte | `mention_everyone` | *(réservé)* |
| Vocal | `connect`, `speak`, `mute_members`, `move_members` | *(réservés pour la feature vocal)* |

### Sémantique

- **Owner** : possède implicitement toutes les permissions, ne peut être ni kické ni rétrogradé. `DELETE /guilds/<gid>` reste owner-only.
- **Socle par défaut** (codé en dur, pas de rôle `@everyone` matérialisé) : tout membre possède
  `view_channels`, `send_messages`, `attach_files`, `create_invite`, `connect`, `speak`.
- **Cumul** : permissions effectives = socle ∪ permissions de tous les rôles assignés (union).
- **`administrator`** : équivaut à toutes les permissions et contourne la hiérarchie.

### Hiérarchie des rôles

Convention : **`position` plus petite = rôle plus élevé** (0 = le plus haut). Le tri `ORDER BY position ASC` de `GET /roles` affiche donc du plus haut au plus bas. Règles (contournées par owner et `administrator`) :

- Créer/modifier/supprimer un rôle : uniquement si sa `position` est strictement inférieure (numériquement supérieure) à son propre rôle le plus élevé — y compris la `position` cible d'un déplacement.
- Assigner/retirer un rôle : uniquement des rôles strictement inférieurs au sien.
- Accorder une permission : impossible d'ajouter à un rôle une permission qu'on ne possède pas soi-même.
- `kick_members` : seulement si le rôle le plus élevé de la cible est strictement inférieur au sien.

### Format d'erreur stable (HTTP)

| Cas | Status | Corps |
|---|---|---|
| Permission manquante | `403` | `{"error": "missing_permission", "permission": "manage_channels"}` |
| Non-membre du serveur | `403` | `{"error": "not_member"}` |
| Violation de hiérarchie | `403` | `{"error": "role_hierarchy"}` |
| Permission inconnue dans `permissions` | `400` | `{"error": "unknown_permissions", "permissions": ["valeur_refusee"]}` |

Sur le WebSocket, le rejet passe par l'événement `error` existant avec un code stable en `content` : `missing_permission:<permission_id>` ou `not_member`.

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

**Erreurs**
- `403` — `{"error": "not_member"}` (DM dont on n'est pas destinataire) ou `{"error": "missing_permission", "permission": "view_channels"}` (channel d'un serveur dont on n'est pas membre)
- `404` — channel introuvable

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

Génère un code d'invitation. Requiert la permission `create_invite` (incluse dans le socle par défaut : tout membre l'a, sauf configuration contraire future).

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

Met à jour le nom et/ou l'icône d'un serveur. Requiert la permission `manage_guild`. Les champs absents ou `null` sont conservés.

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
- `403` — `missing_permission:manage_guild` ou `not_member` (format JSON, voir Permissions)
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

### `GET /guilds/<guild_id>/members/me` 🔒

Retourne le profil de membre de l'utilisateur connecté **et ses permissions effectives calculées côté serveur** (socle ∪ rôles ; vocabulaire complet pour owner/admin).

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |

**Retour** `200`
```json
{
  "member": {
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
  },
  "permissions": ["attach_files", "connect", "create_invite", "send_messages", "speak", "view_channels"]
}
```

**Erreurs**
- `403` — `{"error": "not_member"}`
- `404` — serveur introuvable

---

### `POST /guilds/<guild_id>/members/<user_id>/kick` 🔒

Expulse un membre du serveur. Requiert la permission `kick_members` ; le rôle le plus élevé de la cible doit être strictement inférieur à celui de l'appelant (sauf owner/admin).

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `user_id` | string | `user:<id>` de la cible |

**Retour** `204`

**Erreurs**
- `400` — tentative d'expulser le propriétaire ou soi-même
- `403` — `missing_permission:kick_members` ou `role_hierarchy` (format JSON, voir Permissions)
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

Liste les invitations actives d'un serveur. Requiert la permission `manage_invites`.

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

Révoque une invitation. Requiert la permission `manage_invites`.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `invite_id` | string | `guild_invite:<id>` |

**Retour** `204`

**Erreurs**
- `403` — `missing_permission:manage_invites` (format JSON, voir Permissions)
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

Crée un channel dans un serveur. Requiert la permission `manage_channels` (⚠️ n'est plus ouvert à tout membre).

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
  "created_at": "datetime",
  "permission_overwrites": []
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

Les channels sur lesquels l'utilisateur n'a pas la permission effective `view_channels` (deny via `permission_overwrites` non compensé par un allow) sont **exclus** de la réponse. Le nombre de channels visibles peut donc varier par membre.

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
    "created_at": "datetime",
    "permission_overwrites": [
      { "target": "role:<id>", "allow": ["view_channels"], "deny": [] }
    ]
  }
]
```

---

### `PUT /guilds/<guild_id>/channels/<channel_id>/permissions` 🔒

Remplace la liste entière des overrides de permissions du channel. Requiert la permission `manage_channels`.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `channel_id` | string | `channel:<id>` |

**Corps**
```json
{
  "permission_overwrites": [
    { "target": "role:<id>", "allow": ["view_channels"], "deny": [] },
    { "target": "user:<id>", "allow": [], "deny": ["view_channels"] }
  ]
}
```

| Champ | Type | Description |
|---|---|---|
| `target` | string | `role:<id>` ou `user:<id>` |
| `allow` | string[] | Permissions accordées sur ce channel |
| `deny` | string[] | Permissions retirées sur ce channel |

**Résolution des permissions effectives sur un channel** (priorité croissante) :
1. Socle + rôles (permissions effectives globales)
2. `deny` des rôles du membre
3. `allow` des rôles du membre
4. `deny` de l'utilisateur spécifique
5. `allow` de l'utilisateur spécifique
6. `administrator` (et le propriétaire) contourne toujours tout

**Retour** `200` — l'objet Channel mis à jour (avec `permission_overwrites` peuplé).

**Erreurs**
- `403` — `missing_permission:manage_channels` (format JSON, voir Permissions)
- `400` — `{"error":"unknown_permissions","permissions":[...]}` si une permission de `allow`/`deny` est inconnue
- `400` — `{"error":"invalid_target","target":"..."}` si `target` n'est pas un `role:<id>` ou `user:<id>`
- `404` — channel introuvable ou n'appartenant pas à ce serveur

**Notification WS** envoyée à tous les membres du serveur :
```json
{
  "message_type": "channel_permissions_updated",
  "content": "{ ...Channel }"
}
```

---

### `DELETE /guilds/<guild_id>/channels/<channel_id>` 🔒

Supprime un channel et tous ses messages. Requiert la permission `manage_channels`.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `channel_id` | string | `channel:<id>` |

**Retour** `204`

**Erreurs**
- `403` — `missing_permission:manage_channels` (format JSON, voir Permissions)
- `404` — channel introuvable ou n'appartenant pas à ce serveur

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

Crée un rôle dans un serveur. Requiert la permission `manage_roles`. Les `permissions` sont validées contre le vocabulaire (voir Permissions) ; la `position` doit être strictement inférieure au rôle le plus élevé de l'appelant et il est impossible d'accorder une permission qu'on ne possède pas (sauf owner/admin).

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

**Erreurs**
- `400` — `{"error": "unknown_permissions", "permissions": [...]}`
- `403` — `missing_permission:manage_roles` ou `role_hierarchy` (format JSON, voir Permissions)

**Notification WS** envoyée à tous les membres du serveur :
```json
{
  "message_type": "role_created",
  "content": "{ ...Role }"
}
```

---

### `PATCH /guilds/<guild_id>/roles/<role_id>` 🔒

Modifie un rôle existant. Requiert la permission `manage_roles`. Tous les champs sont optionnels — champ absent = conservé. Mêmes règles de validation et de hiérarchie que la création (rôle cible **et** nouvelle `position` strictement inférieurs au rôle le plus élevé de l'appelant ; impossible d'ajouter une permission qu'on ne possède pas). Les valeurs hors vocabulaire qui traîneraient en base sont purgées à cette occasion.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `role_id` | string | `role:<id>` |

**Corps**
```json
{
  "name": "string",
  "color": "string",
  "position": 1,
  "permissions": ["kick_members", "manage_messages"]
}
```

**Retour** `200` — l'objet Role complet mis à jour (même forme que `GET /roles`).

**Erreurs**
- `400` — `{"error": "unknown_permissions", "permissions": [...]}`
- `403` — `missing_permission:manage_roles` ou `role_hierarchy` (format JSON, voir Permissions)
- `404` — rôle introuvable ou n'appartenant pas à ce serveur

**Notification WS** envoyée à tous les membres du serveur :
```json
{
  "message_type": "role_modified",
  "content": "{ ...Role }"
}
```

---

### `GET /guilds/<guild_id>/roles` 🔒

Liste les rôles d'un serveur, triés par position croissante (du plus haut au plus bas dans la hiérarchie). Réservé aux membres.

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

Supprime un rôle et le retire de tous les membres. Requiert la permission `manage_roles` ; le rôle doit être strictement inférieur au rôle le plus élevé de l'appelant.

**Paramètres URL**
| Paramètre | Type | Description |
|---|---|---|
| `guild_id` | string | `guild:<id>` |
| `role_id` | string | `role:<id>` |

**Retour** `204`

**Erreurs**
- `403` — `missing_permission:manage_roles` ou `role_hierarchy` (format JSON, voir Permissions)
- `404` — rôle introuvable ou n'appartenant pas à ce serveur

**Notification WS** envoyée à tous les membres du serveur :
```json
{
  "message_type": "role_deleted",
  "content": "{\"guild_id\": \"guild:<id>\", \"role_id\": \"role:<id>\"}"
}
```

---

### `POST /guilds/<guild_id>/members/<user_id>/roles/<role_id>` 🔒

Assigne un rôle à un membre. Requiert la permission `manage_roles` ; le rôle doit être strictement inférieur au rôle le plus élevé de l'appelant.

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

Retire un rôle d'un membre. Requiert la permission `manage_roles` ; le rôle doit être strictement inférieur au rôle le plus élevé de l'appelant.

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
  "friends_online": ["user:<id>"],
  "voice_states": [
    {
      "user": { "id": "user:<id>", "name": "string", "display_name": "string", "profile_picture": "string" },
      "guild_id": "guild:<id>",
      "channel_id": "channel:<id>"
    }
  ]
}
```

`friends_online` : liste des IDs d'amis déjà connectés au moment de l'auth. Permet d'initialiser l'état de présence sans polling.

`voice_states` : état courant de la présence vocale dans tous les serveurs dont l'utilisateur est membre. Permet d'initialiser l'affichage des channels vocaux ; à maintenir ensuite via les événements `voice_state_update`.

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
| `channel:<id>` | Message dans un channel de serveur. Requiert d'être membre du serveur + la permission `send_messages` ; `attach_files` en plus si `attachments` est non vide. |

En cas de rejet sur `channel:<id>`, le serveur répond à l'expéditeur avec l'événement `error` et un code stable en `content` :
```json
{ "message_type": "error", "content": "missing_permission:send_messages" }
```
Codes possibles : `not_member`, `missing_permission:send_messages`, `missing_permission:attach_files`, `channel_not_found`, `invalid_channel`.

#### Message relay (P2P signaling)

Quand `message_type` vaut `"relay"`, le serveur transmet le message directement au destinataire **sans le persister en base**. Conçu pour l'échange de signaux WebRTC (SDP/ICE).

**Client → Serveur**
```json
{
  "to": "user:<id>",
  "message_type": "relay",
  "content": "<SDP ou ICE candidate>"
}
```

Requiert une amitié `accepted` avec la cible. Si la cible n'est pas connectée, le message est silencieusement ignoré.

**Serveur → Client destinataire**
```json
{
  "to": "user:<id>",
  "message_type": "relay",
  "from": "user:<sender_id>",
  "content": "<SDP ou ICE candidate>",
  "attachments": []
}
```

---

### Channels vocaux

Le serveur ne transporte pas l'audio (WebRTC peer-to-peer côté clients, signaux SDP/ICE via `relay`) — il ne fait que suivre et diffuser la présence vocale.

#### Rejoindre un channel vocal

**Client → Serveur**
```json
{ "message_type": "voice_join", "channel_id": "channel:<id>" }
```

Enregistre l'utilisateur dans le channel vocal. S'il était déjà dans un autre channel vocal (même serveur ou non), il en est retiré automatiquement (avec diffusion d'un `voice_state_update` à `channel_id: null` dans l'ancien serveur si différent).

**Validation :**
- `channel_id` doit être un channel `Voice` d'un serveur dont l'utilisateur est membre
- Requiert la permission effective `connect` sur le channel (socle par défaut, modifiable par `permission_overwrites`)

En cas de rejet, le serveur répond avec l'événement `error` et un code stable en `content` : `missing_permission:connect`, `not_member`, `not_voice_channel`, `channel_not_found`, `invalid_channel`.
```json
{ "message_type": "error", "content": "missing_permission:connect" }
```

#### Quitter le channel vocal

**Client → Serveur**
```json
{ "message_type": "voice_leave" }
```

Retire l'utilisateur de son channel vocal actuel. No-op s'il n'est dans aucun channel.

#### `voice_state_update` (Serveur → tous les membres du serveur)

Diffusé dès qu'un utilisateur rejoint ou quitte un channel vocal.

```json
{
  "message_type": "voice_state_update",
  "content": "{\"user\":{\"id\":\"user:<id>\",\"name\":\"alice\",\"display_name\":\"Alice\",\"profile_picture\":\"\"},\"guild_id\":\"guild:<id>\",\"channel_id\":\"channel:<id>\"}"
}
```

Sémantique du champ `content` (JSON stringifié) :

| Champ | Type | Description |
|---|---|---|
| `user` | SimpleUser | Infos de l'utilisateur (id, name, display_name, profile_picture) |
| `guild_id` | `string` | ID du serveur concerné |
| `channel_id` | `string \| null` | ID du channel rejoint, `null` si l'utilisateur a quitté le vocal |

**Déclenchement :**
- `voice_join` : `channel_id` = le channel rejoint
- `voice_leave` : `channel_id` = `null`
- Déconnexion du WS : `channel_id` = `null` (cleanup immédiat)
- Kick ou départ du serveur : `channel_id` = `null`
- Suppression du channel vocal : `channel_id` = `null` pour chaque occupant

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

**Permissions d'un channel modifiées** — quand les overrides sont remplacés via `PUT /guilds/<gid>/channels/<cid>/permissions` (le client doit recalculer la visibilité et les permissions effectives du channel)
```json
{
  "message_type": "channel_permissions_updated",
  "content": "{ ...Channel }"
}
```

**Rôle assigné/retiré** — quand un rôle est assigné ou retiré à un membre
```json
{
  "message_type": "role_updated",
  "content": "{\"guild_id\": \"guild:<id>\", \"user_id\": \"user:<id>\", \"role_id\": \"role:<id>\", \"action\": \"assigned | removed\"}"
}
```

**Rôle créé** — quand un rôle est créé via `POST /guilds/<gid>/roles`
```json
{
  "message_type": "role_created",
  "content": "{ ...Role }"
}
```

**Rôle modifié** — quand un rôle est modifié via `PATCH /guilds/<gid>/roles/<rid>` (le client doit recalculer les permissions effectives)
```json
{
  "message_type": "role_modified",
  "content": "{ ...Role }"
}
```

**Rôle supprimé** — quand un rôle est supprimé via `DELETE /guilds/<gid>/roles/<rid>`
```json
{
  "message_type": "role_deleted",
  "content": "{\"guild_id\": \"guild:<id>\", \"role_id\": \"role:<id>\"}"
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
