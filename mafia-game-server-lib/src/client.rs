//! Manages connection to the server. Clients may or may not be players, e.g. if a client connects
//! while a game is ongoing they will not be a player in the game.
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use bit_set::BitSet;
use mafia_game_lib::ClientId;
use mafia_game_lib::ClientInfo;
use mafia_game_lib::Event;
use mafia_game_lib::SessionToken;

use crate::error::MafiaGameError;

pub const MAX_PLAYERS: usize = 64;

/// State for a connected client.
pub(crate) struct Client {
    inbox: Mutex<VecDeque<Arc<Event>>>,
    info: ClientInfo,
    session_token: SessionToken,
    /// Seconds since unix epoch.
    last_active: AtomicU64,
    disconnected: bool,
}

impl Client {
    pub(crate) fn get_info(&self) -> &ClientInfo {
        &self.info
    }
}

#[derive(Clone, Debug)]
pub struct ClientSet(BitSet);

impl From<ClientId> for ClientSet {
    fn from(value: ClientId) -> Self {
        let mut v = BitSet::with_capacity(MAX_PLAYERS);
        v.insert(value.0);

        ClientSet(v)
    }
}

impl ClientSet {
    pub fn new() -> Self {
        ClientSet(BitSet::with_capacity(MAX_PLAYERS))
    }

    pub fn intersect_with(&mut self, other: &Self) {
        self.0.intersect_with(&other.0)
    }

    pub fn union_with(&mut self, other: &Self) {
        self.0.union_with(&other.0)
    }

    pub fn difference_with(&mut self, other: &Self) {
        self.0.difference_with(&other.0)
    }

    pub fn insert(&mut self, client_id: ClientId) -> bool {
        self.0.insert(client_id.0)
    }

    pub fn remove(&mut self, client_id: ClientId) -> bool {
        self.0.remove(client_id.0)
    }

    pub fn count(&self) -> usize {
        self.0.len()
    }
}

impl Default for ClientSet {
    fn default() -> Self {
        Self::new()
    }
}

impl FromIterator<ClientId> for ClientSet {
    fn from_iter<T: IntoIterator<Item = ClientId>>(iter: T) -> Self {
        let mut v = BitSet::with_capacity(MAX_PLAYERS);

        for client_id in iter {
            v.insert(client_id.0);
        }

        ClientSet(v)
    }
}

impl<'a> IntoIterator for &'a ClientSet {
    type Item = ClientId;
    type IntoIter = std::iter::Map<bit_set::Iter<'a, u32>, fn(usize) -> ClientId>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter().map(ClientId)
    }
}

pub(crate) struct ClientState {
    /// Holds state for connected clients.
    clients: HashMap<ClientId, Client>,
    /// Holds mapping of client names to IDs, can hold stale client names.
    client_name_to_id: HashMap<Arc<str>, ClientId>,
    session_token_to_id: HashMap<SessionToken, ClientId>,
    claimed_ids: BitSet,
}

impl Default for ClientState {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientState {
    pub(crate) fn new() -> Self {
        Self {
            clients: HashMap::new(),
            client_name_to_id: HashMap::new(),
            session_token_to_id: HashMap::new(),
            claimed_ids: BitSet::with_capacity(MAX_PLAYERS),
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

            if !client.disconnected {
                return Err(MafiaGameError::ClientNameRegistered(
                    client_name.to_string(),
                ));
            } else {
                let session_token = SessionToken::new();

                client.session_token = session_token;
                client.last_active.store(
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .expect("now is after epoch")
                        .as_secs(),
                    Ordering::Relaxed,
                );
                client.disconnected = false;

                return Ok((existing_client_id, session_token));
            }
        }

        let mut new_id: Option<usize> = None;
        for i in 0..MAX_PLAYERS {
            if !self.claimed_ids.contains(i) {
                new_id = Some(i);
                break;
            }
        }

        let id = ClientId(new_id.ok_or(MafiaGameError::TooManyClientsRegistered)?);
        self.claimed_ids.insert(id.0);

        let session_token = SessionToken::new();

        let client = Client {
            inbox: Mutex::new(VecDeque::with_capacity(100)),
            info: ClientInfo {
                name: Arc::clone(&client_name),
                id,
            },
            session_token,
            last_active: AtomicU64::new(
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("now is after epoch")
                    .as_secs(),
            ),
            disconnected: false,
        };

        self.clients.insert(id, client);
        self.client_name_to_id.insert(client_name, id);
        self.session_token_to_id.insert(session_token, id);

        Ok((id, session_token))
    }

    /// Disconnects the client from the game.
    pub(crate) fn disconnect_client(&mut self, client_id: ClientId) -> Result<(), MafiaGameError> {
        let Some(client) = self.clients.get_mut(&client_id) else {
            return Err(MafiaGameError::InvalidClientId(client_id));
        };

        if client.disconnected {
            return Err(MafiaGameError::ClientDisconnected(client_id));
        }

        client.disconnected = true;
        client.inbox = Mutex::new(VecDeque::with_capacity(100));

        Ok(())
    }

    /// Purges disconnect clients from the client name map.
    ///
    /// Returns a list of clients newly disconnected.
    pub(crate) fn purge_disconnected_clients(
        &mut self,
        max_inactive_time: Duration,
    ) -> Vec<ClientId> {
        let now = SystemTime::now();

        let mut ret = Vec::new();

        for client_id in self
            .clients
            .values()
            .filter_map(|client| {
                if client.disconnected
                    || now
                        .duration_since(
                            UNIX_EPOCH
                                + Duration::from_secs(client.last_active.load(Ordering::Relaxed)),
                        )
                        .unwrap_or(Duration::from_secs(0))
                        >= max_inactive_time
                {
                    Some(client.info.id)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
        {
            let client = self.clients.remove(&client_id).expect("client exists");

            self.client_name_to_id.remove(&client.info.name);
            self.session_token_to_id.remove(&client.session_token);
            self.claimed_ids.remove(client_id.0);

            if !client.disconnected {
                ret.push(client_id);
            }
        }

        ret
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

        if client.disconnected {
            return Err(MafiaGameError::ClientDisconnected(client_id));
        }

        client.last_active.store(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("now is after epoch")
                .as_secs(),
            Ordering::Relaxed,
        );

        Ok(client_id)
    }

    pub(crate) fn get_client(&self, client_id: ClientId) -> Result<&Client, MafiaGameError> {
        self.clients
            .get(&client_id)
            .ok_or(MafiaGameError::InvalidClientId(client_id))
    }

    pub(crate) fn list_clients(&self) -> &HashMap<Arc<str>, ClientId> {
        &self.client_name_to_id
    }

    pub(crate) fn all_client_ids(&self) -> ClientSet {
        ClientSet(self.claimed_ids.clone())
    }

    pub(crate) fn all_client_info(&self) -> HashMap<ClientId, ClientInfo> {
        self.clients
            .iter()
            .map(|(&k, v)| (k, v.info.clone()))
            .collect()
    }

    /// Send a [`Event`] to the specified client's inboxes, if they exist.
    pub(crate) fn send_event<E: Into<Event>>(&self, to: ClientSet, event: E) {
        let event = Arc::new(event.into());

        if to.0.len() < self.clients.len() {
            for id in &to.0 {
                let client_id = ClientId(id);

                if let Some(client) = self.clients.get(&client_id) {
                    if !client.disconnected {
                        client.inbox.lock().unwrap().push_back(Arc::clone(&event));
                    }
                }
            }
        } else {
            for (&client_id, client) in &self.clients {
                if !client.disconnected && to.0.contains(client_id.0) {
                    client.inbox.lock().unwrap().push_back(Arc::clone(&event));
                }
            }
        }
    }

    /// Drains a given client's event inbox.
    pub(crate) fn take_events(&self, for_client: ClientId) -> Box<[Arc<Event>]> {
        if let Some(client) = self.clients.get(&for_client) {
            client.inbox.lock().unwrap().drain(..).collect()
        } else {
            Box::new([])
        }
    }
}
