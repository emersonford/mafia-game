//! Data structured shared by both the Mafia server and client.

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::Arc;

use uuid::Uuid;

/// Identifier for a connected client.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ClientId(pub usize);

/// Unique token to auth a client to the server.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SessionToken(pub Uuid);

impl SessionToken {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionToken {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Which side a player is on.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Allegiance {
    Mafia,
    Villagers,
}

/// A special role a player can be.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum SpecialRole {
    Mafia,
    /// Protects one player from Mafia death each night.
    Doctor,
    /// Investigates the allegiance of one player each night.
    Detective,
}

impl SpecialRole {
    pub fn allegiance(&self) -> Allegiance {
        match self {
            SpecialRole::Mafia => Allegiance::Mafia,
            SpecialRole::Doctor => Allegiance::Villagers,
            SpecialRole::Detective => Allegiance::Villagers,
        }
    }
}

/// State of a client in a game.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum PlayerStatus {
    Alive,
    Dead,
}

/// The current cycle the game is in.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Cycle {
    Day,
    Night,
}

impl Cycle {
    pub fn next(self) -> Self {
        match self {
            Self::Night => Self::Day,
            Self::Day => Self::Night,
        }
    }
}

/// Public information about a client.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ClientInfo {
    pub name: Arc<str>,
    pub id: ClientId,
}

/// Public information about a game.
///
/// This can vary depending on the client's status in the game.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct GameInfo {
    pub cycle_start_time_unix_ts_secs: u64,
    pub cycle_duration_secs: u64,
    pub current_cycle: Cycle,
    pub day_num: usize,
    pub player_to_role: HashMap<ClientId, SpecialRole>,
    pub player_status: HashMap<ClientId, PlayerStatus>,
    pub votes: HashMap<ClientId, Option<ClientId>>,
    pub winner: Option<Allegiance>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ServerInfo {
    pub connected_clients: Vec<ClientInfo>,
    pub active_game: Option<GameInfo>,
}

/// Actor for messages.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Entity {
    Client(ClientId),
    System,
}

/// Channel an event is broadcasted in.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum EventChannel {
    /// Everyone can view this event.
    Public,
    /// Only Mafia, spectators, and dead clients can view this event.
    Mafia,
    /// Only spectators / dead clients can view this event.
    Spectator,
}

/// Message to display to the client's chatbox.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Message {
    pub channel: EventChannel,
    pub contents: Box<str>,
    pub from: Entity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Set the entire server info state, used on first connection.
    SetServerInfo(ServerInfo),
    SetGame(GameInfo),
    EndGame,
    ClientConnected(ClientInfo),
    ClientDisconnected(ClientId),
    MessageReceived(Message),
    VoteIssued {
        voter: ClientId,
        target: Option<ClientId>,
        channel: EventChannel,
    },
    // Events from a cycle end.
    FailedVote {
        cycle: Cycle,
        channel: EventChannel,
    },
    SetCycle {
        cycle: Cycle,
        day_num: usize,
    },
    PlayerKilled {
        player: ClientId,
        cycle: Cycle,
        death_message: Box<str>,
    },
    PlayerInvestigated {
        actor: ClientId,
        target: ClientId,
        allegiance: Allegiance,
    },
    GameWon {
        player_to_role: HashMap<ClientId, SpecialRole>,
        side: Allegiance,
    },
}

impl From<Message> for Event {
    fn from(value: Message) -> Self {
        Event::MessageReceived(value)
    }
}
