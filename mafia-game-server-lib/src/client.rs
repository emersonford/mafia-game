//! Manages connection to the server. Clients may or may not be players, e.g. if a client connects
//! while a game is ongoing they will not be a player in the game.
use std::collections::HashMap;
use std::collections::VecDeque;
use std::fmt::Display;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use rand::seq::SliceRandom;
use uuid::Uuid;

use crate::consts::PLAYER_EMOJIS;
use crate::error::MafiaGameError;

/// Identifier for a connected client.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ClientId(pub usize);

/// Actor for messages.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Entity {
    Client(ClientId),
    System,
}

/// Unique token to auth a client to the server.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SessionToken(pub Uuid);

impl Display for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Channel a message is broadcasted in.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum MessageChannel {
    /// Everyone can view this message.
    Public,
    /// Only Mafia can view this message.
    Mafia,
    /// Only spectators / dead clients can view this message.
    Spectator,
}

/// Message to display to the client's chatbox.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Message {
    pub channel: MessageChannel,
    pub contents: Box<str>,
    pub from: Entity,
}

/// State for a connected client.
pub(crate) struct Client {
    message_inbox: Mutex<VecDeque<Arc<Message>>>,
    name: Arc<str>,
    session_token: SessionToken,
    id: ClientId,
    emoji: char,
    /// Seconds since unix epoch.
    last_active: AtomicU64,
    disconnected: AtomicBool,
}

impl Client {
    #[cfg(test)]
    pub(crate) fn get_name(&self) -> &str {
        &self.name
    }
}

pub(crate) struct ClientState {
    /// Holds state for connected clients.
    clients: HashMap<ClientId, Client>,
    /// Holds mapping of client names to IDs, can hold stale client names.
    client_name_to_id: HashMap<Arc<str>, ClientId>,
    session_token_to_id: HashMap<SessionToken, ClientId>,
    next_id: ClientId,
    available_emoji: VecDeque<char>,
}

impl Default for ClientState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientState {
    pub(crate) fn new() -> Self {
        let mut available_emoji = PLAYER_EMOJIS.to_vec();
        let mut rng = rand::rng();
        available_emoji.shuffle(&mut rng);

        Self {
            clients: HashMap::new(),
            client_name_to_id: HashMap::new(),
            session_token_to_id: HashMap::new(),
            next_id: ClientId(0),
            available_emoji: available_emoji.into_iter().collect(),
        }
    }

    pub(crate) fn connect_client(
        &mut self,
        client_name: &str,
    ) -> Result<(ClientId, SessionToken), MafiaGameError> {
        let client_name = Arc::from(client_name);

        if let Some(&existing_client_id) = self.client_name_to_id.get(&client_name) {
            let client = self
                .clients
                .get_mut(&existing_client_id)
                .expect("client exists");

            if !client.disconnected.load(Ordering::Relaxed) {
                return Err(MafiaGameError::ClientNameRegistered(
                    client_name.to_string(),
                ));
            } else {
                let session_token = SessionToken(Uuid::new_v4());

                client.session_token = session_token;
                client.last_active.store(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("now is after epoch")
                        .as_secs(),
                    Ordering::Relaxed,
                );
                client.disconnected.store(false, Ordering::Relaxed);

                return Ok((existing_client_id, session_token));
            }
        }

        let id = self.next_id;
        self.next_id = ClientId(self.next_id.0 + 1);

        let session_token = SessionToken(Uuid::new_v4());

        let client = Client {
            message_inbox: Mutex::new(VecDeque::with_capacity(100)),
            name: Arc::clone(&client_name),
            session_token,
            id,
            last_active: AtomicU64::new(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("now is after epoch")
                    .as_secs(),
            ),
            emoji: self
                .available_emoji
                .pop_front()
                .ok_or(MafiaGameError::TooManyClientsRegistered)?,
            disconnected: AtomicBool::new(false),
        };

        self.clients.insert(id, client);
        self.client_name_to_id.insert(client_name, id);
        self.session_token_to_id.insert(session_token, id);

        Ok((id, session_token))
    }

    /// Disconnects the client from the game.
    pub(crate) fn disconnect_client(&self, client_id: ClientId) -> Result<(), MafiaGameError> {
        let Some(client) = self.clients.get(&client_id) else {
            return Err(MafiaGameError::InvalidClientId(client_id));
        };

        client.disconnected.store(true, Ordering::Relaxed);

        Ok(())
    }

    /// Purges disconnect clients from the client name map.
    pub(crate) fn purge_disconnected_clients(&mut self, max_inactive_time: Duration) {
        let now = SystemTime::now();

        for client_id in self
            .clients
            .values()
            .filter_map(|client| {
                if client.disconnected.load(Ordering::Relaxed)
                    || now
                        .duration_since(
                            UNIX_EPOCH
                                + Duration::from_secs(client.last_active.load(Ordering::Relaxed)),
                        )
                        .unwrap_or(Duration::from_secs(0))
                        >= max_inactive_time
                {
                    Some(client.id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
        {
            let client = self.clients.remove(&client_id).expect("client exists");

            self.client_name_to_id.remove(&client.name);
            self.session_token_to_id.remove(&client.session_token);
            self.available_emoji.push_back(client.emoji);
        }
    }

    /// Returns the [`ClientId`] associated with the given session token, effectively
    /// authenticating the session token.
    pub(crate) fn auth_client(
        &self,
        session_token: SessionToken,
    ) -> Result<ClientId, MafiaGameError> {
        let client_id = self
            .session_token_to_id
            .get(&session_token)
            .copied()
            .ok_or(MafiaGameError::InvalidSessionToken(session_token))?;

        let client = self.clients.get(&client_id).expect("valid client");

        client.last_active.store(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("now is after epoch")
                .as_secs(),
            Ordering::Relaxed,
        );
        client.disconnected.store(false, Ordering::Relaxed);

        Ok(client_id)
    }

    #[cfg(test)]
    pub(crate) fn get_client(&self, client_id: ClientId) -> Result<&Client, MafiaGameError> {
        self.clients
            .get(&client_id)
            .ok_or(MafiaGameError::InvalidClientId(client_id))
    }

    pub(crate) fn list_clients(&self) -> &HashMap<Arc<str>, ClientId> {
        &self.client_name_to_id
    }

    /// Send a [`Message`] to the specified client's inboxes, if they exist.
    pub(crate) fn send_message(&self, to: &[ClientId], msg: Message) {
        let msg = Arc::new(msg);

        for id in to {
            if let Some(client) = self.clients.get(id) {
                client
                    .message_inbox
                    .lock()
                    .unwrap()
                    .push_back(Arc::clone(&msg));
            }
        }
    }

    /// Drains a given client's message inbox.
    pub(crate) fn take_messages(&self, for_client: ClientId) -> Box<[Arc<Message>]> {
        if let Some(client) = self.clients.get(&for_client) {
            client.message_inbox.lock().unwrap().drain(..).collect()
        } else {
            Box::new([])
        }
    }
}
