use crate::chat::types::{ChatMessage, PresenceEvent, ServerMessage};
use crate::friends::are_accepted_friends;
use crate::guild_channels::get_guild_member_ids;
use crate::guilds::share_guild;
use crate::messages::save_message_with_author;
use crate::models::db::{DMChannel, MemberOf, SimpleUser};
use crate::permissions::{check_channel_send, check_voice_join};
use rocket::tokio::sync::RwLock;
use rocket::tokio::sync::broadcast::Sender;
use std::collections::{HashMap, HashSet};
use surrealdb::Surreal;
use surrealdb::engine::any::Any;
use surrealdb::sql::Thing;

#[derive(Clone, Debug)]
pub struct VoiceState {
    pub guild_id: String,
    pub channel_id: String,
}

pub struct ChatHub {
    pub connections: RwLock<HashMap<String, Sender<String>>>,
    pub voice_states: RwLock<HashMap<String, VoiceState>>,
}

async fn fetch_simple_user(db: &Surreal<Any>, user_id: &str) -> Option<SimpleUser> {
    let user_thing = surrealdb::sql::thing(user_id).ok()?;
    db.query("SELECT id, name, display_name, profile_picture FROM user WHERE id = $id")
        .bind(("id", user_thing))
        .await
        .ok()?
        .take::<Vec<SimpleUser>>(0)
        .ok()?
        .pop()
}

impl ChatHub {
    pub fn new() -> Self {
        ChatHub {
            connections: RwLock::new(HashMap::new()),
            voice_states: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_dm_channel(
        &self,
        db: &Surreal<Any>,
        user_ids: Vec<String>,
        owner_id: String,
    ) -> Option<String> {
        let user_things: Vec<Thing> = user_ids
            .iter()
            .filter_map(|s| surrealdb::sql::thing(s).ok())
            .collect();

        if user_things.len() != user_ids.len() {
            return None;
        }

        let owner_thing = surrealdb::sql::thing(&owner_id).ok()?;

        // Check if a DMChannel already exists for these users
        let query = "SELECT * FROM DMChannel WHERE recipients CONTAINSALL $user_ids AND array::len(recipients) = $num_users";
        let result = db
            .query(query)
            .bind(("user_ids", user_things.clone()))
            .bind(("num_users", user_things.len() as i64))
            .await;

        if let Ok(mut res) = result {
            match res.take::<Vec<DMChannel>>(0) {
                Ok(mut existing) => {
                    if let Some(dm) = existing.pop() {
                        return dm.id.map(|id| id.to_raw());
                    }
                }
                Err(e) => eprintln!("SELECT deserialization error: {:?}", e),
            }
        }

        let mut sorted_ids: Vec<String> = user_things.iter().map(|t| t.to_raw()).collect();
        sorted_ids.sort();
        let recipients_key = sorted_ids.join(",");

        let create_query = "
            CREATE DMChannel SET
                recipients = $user_ids,
                owner = $owner,
                recipients_key = $recipients_key
        ";

        let create_result = db
            .query(create_query)
            .bind(("user_ids", user_things.clone()))
            .bind(("owner", owner_thing))
            .bind(("recipients_key", recipients_key))
            .await;

        if let Ok(mut res) = create_result {
            match res.take::<Vec<DMChannel>>(0) {
                Ok(mut channels) => {
                    if let Some(new_dm) = channels.pop() {
                        return new_dm.id.map(|id| id.to_raw());
                    }
                }
                Err(e) => {
                    eprintln!(
                        "CREATE deserialization error (possibly unique index hit): {:?}",
                        e
                    );
                    let retry = db
                        .query("SELECT * FROM DMChannel WHERE recipients CONTAINSALL $user_ids AND array::len(recipients) = $num_users")
                        .bind(("user_ids", user_things.clone()))
                        .bind(("num_users", user_things.len() as i64))
                        .await;
                    if let Ok(mut res2) = retry {
                        if let Ok(mut existing) = res2.take::<Vec<DMChannel>>(0) {
                            if let Some(dm) = existing.pop() {
                                return dm.id.map(|id| id.to_raw());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub async fn forward_to_client(&self, user_id: &str, message: &str) {
        let connections = self.connections.read().await;
        if let Some(sender) = connections.get(user_id) {
            let _ = sender.send(message.to_string());
        }
    }

    pub async fn send_to_user(hub: &ChatHub, db: &Surreal<Any>, message: &mut ChatMessage) {
        let sender_id = message.from.clone().unwrap_or_default();
        let target_id = message.to.clone();

        let Ok(sender_thing) = surrealdb::sql::thing(&sender_id) else {
            println!("Invalid sender id: {}", sender_id);
            return;
        };
        let Ok(target_thing) = surrealdb::sql::thing(&target_id) else {
            println!("Invalid target id: {}", target_id);
            return;
        };

        if !are_accepted_friends(db, &sender_id, &target_id).await {
            let error = serde_json::to_string(&ServerMessage {
                message_type: "error".to_string(),
                content: "you must be friends to send a direct message".to_string(),
            })
            .unwrap_or_default();
            ChatHub::forward_to_client(hub, &sender_id, &error).await;
            return;
        }

        let get_dm = db
            .query("SELECT * FROM DMChannel WHERE recipients CONTAINSALL [$sender_id, $target_id] AND array::len(recipients) = 2")
            .bind(("sender_id", sender_thing))
            .bind(("target_id", target_thing))
            .await;

        if let Ok(mut dm_result) = get_dm {
            let dm = dm_result
                .take::<Vec<DMChannel>>(0)
                .ok()
                .and_then(|mut v| v.pop());

            match dm {
                Some(channel) => {
                    if let Some(id) = channel.id {
                        message.to = id.to_raw();
                        ChatHub::send_to_dm_channel(hub, db, message).await;
                    }
                }
                None => {
                    let channel_id = ChatHub::create_dm_channel(
                        hub,
                        db,
                        vec![sender_id.clone(), target_id.clone()],
                        sender_id.clone(),
                    )
                    .await;

                    if let Some(id) = channel_id {
                        message.to = id.clone();
                        ChatHub::send_to_dm_channel(hub, db, message).await;
                        for recipient in vec![sender_id.clone(), target_id.clone()] {
                            if recipient != sender_id {
                                let dm_channel_id_message = ServerMessage {
                                    message_type: "dm_channel_created".to_string(),
                                    content: id.clone(),
                                };
                                ChatHub::forward_to_client(
                                    hub,
                                    &recipient,
                                    &serde_json::to_string(&dm_channel_id_message).unwrap(),
                                )
                                .await;
                            }
                        }
                    } else {
                        println!(
                            "Failed to create DM channel for users: {}, {}",
                            sender_id, target_id
                        );
                    }
                }
            }
        } else {
            println!("Error querying DM Channel: {:?}", get_dm.err());
        }
    }

    pub async fn send_to_dm_channel(
        hub: &ChatHub,
        db: &Surreal<Any>,
        message: &mut ChatMessage,
    ) {
        let sender_id = message.from.clone().unwrap_or_default();
        let dm_channel_id = message.to.clone();

        let Ok(dm_thing) = surrealdb::sql::thing(&dm_channel_id) else {
            println!("Invalid DM channel id: {}", dm_channel_id);
            return;
        };
        let Ok(sender_thing) = surrealdb::sql::thing(&sender_id) else {
            println!("Invalid sender id: {}", sender_id);
            return;
        };

        let get_dm = db
            .query("SELECT * FROM DMChannel WHERE id = $dm_channel_id AND recipients CONTAINS $sender_id")
            .bind(("sender_id", sender_thing))
            .bind(("dm_channel_id", dm_thing))
            .await;

        if let Ok(mut dm_result) = get_dm {
            match dm_result.take::<Vec<DMChannel>>(0) {
                Ok(mut channels) => {
                    if let Some(channel) = channels.pop() {
                        for recipient in &channel.recipients {
                            let recipient_id = recipient.to_raw();
                            if recipient_id == sender_id {
                                continue;
                            }
                            if !are_accepted_friends(db, &sender_id, &recipient_id).await {
                                let error = serde_json::to_string(&ServerMessage {
                                    message_type: "error".to_string(),
                                    content: "you are no longer friends with all participants".to_string(),
                                })
                                .unwrap_or_default();
                                ChatHub::forward_to_client(hub, &sender_id, &error).await;
                                return;
                            }
                        }

                        match save_message_with_author(db, &dm_channel_id, &sender_id, &message.content, message.attachments.clone()).await {
                            Ok(saved) => {
                                if let Some(msg_id) = &saved.id {
                                    let _ = db
                                        .query("UPDATE $channel_id SET last_message_id = $msg_id")
                                        .bind(("channel_id", surrealdb::sql::thing(&dm_channel_id).unwrap()))
                                        .bind(("msg_id", msg_id.clone()))
                                        .await;
                                }

                                let envelope = ServerMessage {
                                    message_type: "new_message".to_string(),
                                    content: serde_json::to_string(&saved).unwrap_or_default(),
                                };
                                let payload = serde_json::to_string(&envelope).unwrap_or_default();

                                for recipient in &channel.recipients {
                                    ChatHub::forward_to_client(hub, &recipient.to_raw(), &payload).await;
                                }
                            }
                            Err(e) => eprintln!("Failed to save message: {e}"),
                        }
                    } else {
                        println!("DM Channel not found or sender is not a recipient");
                    }
                }
                Err(e) => println!("DM Channel deserialization error: {:?}", e),
            }
        } else {
            println!("Error querying DM Channel: {:?}", get_dm.err());
        }
    }

    pub async fn send_to_channel(hub: &ChatHub, db: &Surreal<Any>, message: &mut ChatMessage) {
        let sender_id = message.from.clone().unwrap_or_default();
        let channel_id = message.to.clone();

        let Ok(channel_thing) = surrealdb::sql::thing(&channel_id) else {
            eprintln!("Invalid channel id: {}", channel_id);
            return;
        };

        if let Err(code) =
            check_channel_send(db, &channel_id, &sender_id, !message.attachments.is_empty()).await
        {
            let error = serde_json::to_string(&ServerMessage {
                message_type: "error".to_string(),
                content: code,
            })
            .unwrap_or_default();
            ChatHub::forward_to_client(hub, &sender_id, &error).await;
            return;
        }

        let member_ids = match get_guild_member_ids(db, &channel_thing).await {
            Ok(ids) if !ids.is_empty() => ids,
            Ok(_) => {
                eprintln!("No members found for channel: {}", channel_id);
                return;
            }
            Err(e) => {
                eprintln!("Failed to get guild members: {e}");
                return;
            }
        };

        match save_message_with_author(db, &channel_id, &sender_id, &message.content, message.attachments.clone()).await {
            Ok(saved) => {
                let envelope = ServerMessage {
                    message_type: "new_message".to_string(),
                    content: serde_json::to_string(&saved).unwrap_or_default(),
                };
                let payload = serde_json::to_string(&envelope).unwrap_or_default();

                for member_id in &member_ids {
                    ChatHub::forward_to_client(hub, &member_id.to_raw(), &payload).await;
                }
            }
            Err(e) => eprintln!("Failed to save channel message: {e}"),
        }
    }

    pub async fn relay_to_user(hub: &ChatHub, db: &Surreal<Any>, message: &ChatMessage) {
        let sender_id = message.from.clone().unwrap_or_default();
        let target_id = &message.to;

        if !ChatHub::can_relay(db, &sender_id, target_id).await {
            let error = serde_json::to_string(&ServerMessage {
                message_type: "error".to_string(),
                content: "you must be friends or share a guild to send a relay message".to_string(),
            })
            .unwrap_or_default();
            ChatHub::forward_to_client(hub, &sender_id, &error).await;
            return;
        }

        let payload = serde_json::to_string(message).unwrap_or_default();
        ChatHub::forward_to_client(hub, target_id, &payload).await;
    }

    // Relay (WebRTC signaling) is allowed between accepted friends and between
    // members of a common guild (guild voice channels work without friendship).
    pub async fn can_relay(db: &Surreal<Any>, sender_id: &str, target_id: &str) -> bool {
        are_accepted_friends(db, sender_id, target_id).await
            || share_guild(db, sender_id, target_id).await
    }

    pub async fn send_to(hub: &ChatHub, db: &Surreal<Any>, message: &mut ChatMessage) {
        if message.message_type == "relay" {
            ChatHub::relay_to_user(hub, db, message).await;
            return;
        }

        let parts: Vec<&str> = message.to.split(":").collect();

        let (Some(data_type), Some(_id)) = (parts.get(0), parts.get(1)) else {
            println!("Invalid target format: {}", message.to);
            return;
        };

        match *data_type {
            "user" => ChatHub::send_to_user(hub, db, message).await,
            "DMChannel" => ChatHub::send_to_dm_channel(hub, db, message).await,
            "channel" => ChatHub::send_to_channel(hub, db, message).await,
            other => println!("Unknown data type: {other}"),
        }
    }

    pub async fn broadcast_to_guild_members(&self, db: &Surreal<Any>, guild_id: &str, payload: &str) {
        let Ok(guild_thing) = surrealdb::sql::thing(guild_id) else { return; };

        let memberships: Vec<MemberOf> = match db
            .query("SELECT * FROM member_of WHERE out = $guild_id")
            .bind(("guild_id", guild_thing))
            .await
        {
            Ok(mut res) => res.take(0).unwrap_or_default(),
            Err(_) => return,
        };

        let connections = self.connections.read().await;
        for member in &memberships {
            let member_id = member.user.to_raw();
            if let Some(tx) = connections.get(&member_id) {
                let _ = tx.send(payload.to_string());
            }
        }
    }

    // Returns the serialized presence event for `user_id` based on their current connection state.
    pub async fn presence_event_for(&self, user_id: &str) -> String {
        let connections = self.connections.read().await;
        let online = connections.contains_key(user_id);
        serde_json::to_string(&PresenceEvent {
            message_type: if online { "user_online" } else { "user_offline" }.to_string(),
            user_id: user_id.to_string(),
        })
        .unwrap_or_default()
    }

    // Broadcasts a presence event to all connected friends of `user_id`.
    // Returns the list of friend IDs that are currently connected (useful for initial online state).
    pub async fn broadcast_presence(&self, db: &Surreal<Any>, user_id: &str, online: bool) -> Vec<String> {
        let Ok(user_thing) = surrealdb::sql::thing(user_id) else { return vec![]; };

        let friendships: Vec<crate::models::db::Friendship> = match db
            .query("SELECT * FROM friendship WHERE (`in` = $uid OR out = $uid) AND status = 'accepted'")
            .bind(("uid", user_thing))
            .await
        {
            Ok(mut res) => res.take(0).unwrap_or_default(),
            Err(_) => return vec![],
        };

        let event = serde_json::to_string(&PresenceEvent {
            message_type: if online { "user_online" } else { "user_offline" }.to_string(),
            user_id: user_id.to_string(),
        })
        .unwrap_or_default();

        let connections = self.connections.read().await;
        let mut connected_friends = vec![];

        for f in &friendships {
            let friend_id = if f.user_a.to_raw() == user_id {
                f.user_b.to_raw()
            } else {
                f.user_a.to_raw()
            };
            if let Some(tx) = connections.get(&friend_id) {
                let _ = tx.send(event.clone());
                connected_friends.push(friend_id);
            }
        }

        connected_friends
    }

    async fn broadcast_voice_state(
        &self,
        db: &Surreal<Any>,
        user_id: &str,
        guild_id: &str,
        channel_id: Option<&str>,
    ) {
        let Some(user) = fetch_simple_user(db, user_id).await else { return; };

        let payload = serde_json::json!({
            "user": user,
            "guild_id": guild_id,
            "channel_id": channel_id,
        })
        .to_string();
        let event = serde_json::to_string(&ServerMessage {
            message_type: "voice_state_update".to_string(),
            content: payload,
        })
        .unwrap_or_default();

        self.broadcast_to_guild_members(db, guild_id, &event).await;
    }

    // Registers `user_id` in voice channel `channel_id` after validation.
    // Returns a stable error code on rejection, meant for the WS error event.
    pub async fn voice_join(
        &self,
        db: &Surreal<Any>,
        user_id: &str,
        channel_id: &str,
    ) -> Result<(), String> {
        let channel = check_voice_join(db, channel_id, user_id).await?;
        let guild_id = channel.guild.to_raw();

        let previous = {
            let mut states = self.voice_states.write().await;
            states.insert(
                user_id.to_string(),
                VoiceState {
                    guild_id: guild_id.clone(),
                    channel_id: channel_id.to_string(),
                },
            )
        };

        if let Some(prev) = previous {
            if prev.channel_id == channel_id {
                return Ok(());
            }
            if prev.guild_id != guild_id {
                self.broadcast_voice_state(db, user_id, &prev.guild_id, None).await;
            }
        }

        self.broadcast_voice_state(db, user_id, &guild_id, Some(channel_id)).await;
        Ok(())
    }

    // Removes `user_id` from their current voice channel. No-op if not in one.
    pub async fn voice_leave(&self, db: &Surreal<Any>, user_id: &str) {
        let removed = self.voice_states.write().await.remove(user_id);
        if let Some(state) = removed {
            self.broadcast_voice_state(db, user_id, &state.guild_id, None).await;
        }
    }

    // Removes `user_id` from voice only if they are in a channel of `guild_id`
    // (used when a member is kicked or leaves the guild).
    pub async fn voice_leave_guild(&self, db: &Surreal<Any>, user_id: &str, guild_id: &str) {
        let removed = {
            let mut states = self.voice_states.write().await;
            if states.get(user_id).map(|s| s.guild_id == guild_id) == Some(true) {
                states.remove(user_id)
            } else {
                None
            }
        };
        if removed.is_some() {
            self.broadcast_voice_state(db, user_id, guild_id, None).await;
        }
    }

    // Removes everyone from a deleted voice channel and notifies the guild.
    pub async fn clear_channel_voice_states(
        &self,
        db: &Surreal<Any>,
        guild_id: &str,
        channel_id: &str,
    ) {
        let removed_users: Vec<String> = {
            let mut states = self.voice_states.write().await;
            let users: Vec<String> = states
                .iter()
                .filter(|(_, s)| s.channel_id == channel_id)
                .map(|(uid, _)| uid.clone())
                .collect();
            for uid in &users {
                states.remove(uid);
            }
            users
        };
        for uid in &removed_users {
            self.broadcast_voice_state(db, uid, guild_id, None).await;
        }
    }

    // Drops all voice states of a deleted guild (no broadcast: the
    // `guild_deleted` event already tells clients to discard the guild).
    pub async fn clear_guild_voice_states(&self, guild_id: &str) {
        let mut states = self.voice_states.write().await;
        states.retain(|_, s| s.guild_id != guild_id);
    }

    // Snapshot of the voice presence across all guilds `user_id` belongs to,
    // shaped like the `voice_state_update` content for the `authenticated` response.
    pub async fn voice_states_for_user(
        &self,
        db: &Surreal<Any>,
        user_id: &str,
    ) -> Vec<serde_json::Value> {
        let Ok(user_thing) = surrealdb::sql::thing(user_id) else { return vec![]; };

        let memberships: Vec<MemberOf> = match db
            .query("SELECT * FROM member_of WHERE `in` = $user_id")
            .bind(("user_id", user_thing))
            .await
        {
            Ok(mut res) => res.take(0).unwrap_or_default(),
            Err(_) => return vec![],
        };
        let guild_ids: HashSet<String> = memberships.iter().map(|m| m.guild.to_raw()).collect();

        let snapshot: Vec<(String, VoiceState)> = {
            let states = self.voice_states.read().await;
            states
                .iter()
                .filter(|(_, s)| guild_ids.contains(&s.guild_id))
                .map(|(uid, s)| (uid.clone(), s.clone()))
                .collect()
        };

        let mut result = vec![];
        for (uid, state) in snapshot {
            if let Some(user) = fetch_simple_user(db, &uid).await {
                result.push(serde_json::json!({
                    "user": user,
                    "guild_id": state.guild_id,
                    "channel_id": state.channel_id,
                }));
            }
        }
        result
    }
}
