//! Provides an implementation for the [Mafia game](https://en.wikipedia.org/wiki/Mafia_(party_game)).

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use client::ClientSet;
use client::ClientState;
use consts::DAY_DEATH_MESSAGES;
use consts::NIGHT_DEATH_MESSAGES;
use game::Game;
use game::GameConfig;
use game::is_alive;
use mafia_game_lib::Allegiance;
use mafia_game_lib::ClientId;
use mafia_game_lib::Cycle;
use mafia_game_lib::Entity;
use mafia_game_lib::Event;
use mafia_game_lib::EventChannel;
use mafia_game_lib::GameInfo;
use mafia_game_lib::Message;
use mafia_game_lib::PlayerStatus;
use mafia_game_lib::ServerInfo;
use mafia_game_lib::SessionToken;
use mafia_game_lib::SpecialRole;
use rand::Rng;
use rand::seq::IndexedRandom;

pub mod client;
mod consts;
mod error;
pub mod game;

pub use error::MafiaGameError;
use tap::Tap;

pub struct MafiaGameServerConfig {
    /// Max time a client can be inactive before we force disconnect it.
    pub max_client_inactive_time: Duration,
    pub randomize_death_message: bool,
}

struct MafiaGameServerInner {
    config: MafiaGameServerConfig,
    clients: ClientState,
    active_game: Option<Game>,
}

impl MafiaGameServerInner {
    fn get_active_game(&self) -> Result<&Game, MafiaGameError> {
        let Some(game) = self.active_game.as_ref() else {
            return Err(MafiaGameError::NoGameInProgress);
        };

        if game.get_winner().is_some() {
            return Err(MafiaGameError::NoGameInProgress);
        }

        Ok(game)
    }

    fn get_active_game_mut(&mut self) -> Result<&mut Game, MafiaGameError> {
        let Some(game) = self.active_game.as_mut() else {
            return Err(MafiaGameError::NoGameInProgress);
        };

        if game.get_winner().is_some() {
            return Err(MafiaGameError::NoGameInProgress);
        }

        Ok(game)
    }

    fn in_active_game(&self) -> bool {
        self.active_game
            .as_ref()
            .is_some_and(|game| game.get_winner().is_none())
    }

    fn disconnect_client(&mut self, client_id: ClientId) -> Result<(), MafiaGameError> {
        self.clients.disconnect_client(client_id)?;

        self.clients.send_event(
            self.clients.all_client_ids(),
            Event::ClientDisconnected(client_id),
        );

        Ok(())
    }

    fn purge_disconnected_clients(&mut self) {
        let clients_disconnected = self
            .clients
            .purge_disconnected_clients(self.config.max_client_inactive_time);

        for client_id in clients_disconnected {
            self.clients.send_event(
                self.clients.all_client_ids(),
                Event::ClientDisconnected(client_id),
            );
        }
    }

    fn get_clients_for_channel(&self, actor: Option<ClientId>, channel: EventChannel) -> ClientSet {
        let all_clients = self.clients.all_client_ids();

        match channel {
            EventChannel::Public => all_clients,
            EventChannel::Mafia => {
                if let Some(game) = self.active_game.as_ref() {
                    all_clients.tap_mut(|s| {
                        s.difference_with(&game.get_players(|status, _, allegiance| {
                            status == PlayerStatus::Alive && allegiance == Allegiance::Villagers
                        }));
                    })
                } else {
                    ClientSet::new()
                }
            }
            EventChannel::Spectator => {
                if let Some(game) = self.active_game.as_ref() {
                    all_clients.tap_mut(|s| {
                        s.difference_with(&game.get_players(is_alive));
                    })
                } else {
                    all_clients
                }
            }
        }
        .tap_mut(|s| {
            // Sender can always see their own messages.
            if let Some(client_id) = actor {
                s.insert(client_id);
            }
        })
    }

    /// Returns a set of clients eligible to see the given event.
    fn get_event_visibility(&self, event: &Event) -> ClientSet {
        match event {
            // These events should have their contents tailed to the recipient, hence should not be
            // called in this function.
            Event::SetServerInfo(_) | Event::SetGame(_) => {
                unreachable!("should not be called with `get_event_visibility`")
            }
            Event::EndGame => self.clients.all_client_ids(),
            Event::ClientConnected(info) => self.clients.all_client_ids().tap_mut(|s| {
                s.remove(info.id);
            }),
            Event::ClientDisconnected(client_id) => self.clients.all_client_ids().tap_mut(|s| {
                s.remove(*client_id);
            }),
            Event::MessageReceived(message) => match message.from {
                Entity::Client(client_id) => {
                    self.get_clients_for_channel(Some(client_id), message.channel)
                }
                Entity::System => self.clients.all_client_ids(),
            },
            Event::VoteIssued {
                voter,
                target: _,
                channel,
            } => self.get_clients_for_channel(Some(*voter), *channel),
            Event::FailedVote { cycle: _, channel } => self.get_clients_for_channel(None, *channel),
            Event::PlayerKilled {
                player: _,
                cycle: _,
                death_message: _,
            } => self.clients.all_client_ids(),
            Event::SetCycle {
                start_time_unix_ts_secs: _,
                duration_secs: _,
                cycle: _,
                day_num: _,
            } => self.clients.all_client_ids(),
            Event::PlayerInvestigated {
                actor,
                target: _,
                allegiance: _,
            } => self.get_clients_for_channel(Some(*actor), EventChannel::Spectator),
            Event::GameWon {
                player_to_role: _,
                side: _,
            } => self.clients.all_client_ids(),
        }
    }

    fn send_event(&self, mut event: Event) {
        let to = self.get_event_visibility(&event);

        if let Event::PlayerKilled {
            player: _,
            cycle,
            death_message,
        } = &mut event
        {
            if self.config.randomize_death_message {
                let mut rng = rand::rng();

                match cycle {
                    Cycle::Day => {
                        *death_message =
                            Box::from(*DAY_DEATH_MESSAGES.choose(&mut rng).expect("at least 1"));
                    }
                    Cycle::Night => {
                        *death_message =
                            Box::from(*NIGHT_DEATH_MESSAGES.choose(&mut rng).expect("at least 1"));
                    }
                }
            }
        }

        self.clients.send_event(to, event);
    }

    fn get_game_info_for(&self, client: ClientId) -> Option<GameInfo> {
        let Some(game) = self.active_game.as_ref() else {
            return None;
        };

        let mut game_info = GameInfo {
            cycle_start_time_unix_ts_secs: if cfg!(test) {
                0
            } else {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("now is after epoch")
                    .as_secs()
            },
            cycle_duration_secs: game.get_cycle_duration().as_secs(),
            current_cycle: game.get_cycle(),
            day_num: game.get_day_num(),
            player_status: game.get_player_statuses().clone(),
            winner: game.get_winner(),
            player_to_role: HashMap::new(),
            votes: HashMap::new(),
        };

        let status = game.get_player_status(client);
        let role = game.get_player_role(client);
        let cycle = game.get_cycle();

        match (status, role, cycle) {
            // Spectator or dead person can see everything.
            (None | Some(PlayerStatus::Dead), _, _) => {
                game_info.votes = game.get_votes().clone();
            }
            // Everyone can see votes during the day.
            (Some(PlayerStatus::Alive), _, Cycle::Day) => {
                game_info.votes = game.get_votes().clone();
            }
            // Mafia can see every other mafia's vote.
            (Some(PlayerStatus::Alive), Some(SpecialRole::Mafia), Cycle::Night) => {
                game_info.votes = game
                    .get_votes()
                    .iter()
                    .filter_map(|(k, v)| {
                        if game.get_player_allegiance(*k) == Allegiance::Mafia {
                            Some((*k, *v))
                        } else {
                            None
                        }
                    })
                    .collect();
            }
            // Special role can only see their own votes in the night.
            (Some(PlayerStatus::Alive), Some(_), Cycle::Night) => {
                game_info.votes = game
                    .get_votes()
                    .iter()
                    .filter_map(|(k, v)| if *k == client { Some((*k, *v)) } else { None })
                    .collect();
            }
            // Villagers without roles cannot see any votes in the night.
            (Some(PlayerStatus::Alive), None, Cycle::Night) => {}
        }

        match (status, role) {
            // Spectator or dead person can see everything.
            (None | Some(PlayerStatus::Dead), _) => {
                game_info.player_to_role = game.get_player_roles().clone();
            }
            (Some(PlayerStatus::Alive), Some(SpecialRole::Mafia)) => {
                game_info.player_to_role = game
                    .get_player_roles()
                    .iter()
                    .filter_map(|(&k, &v)| {
                        if v == SpecialRole::Mafia {
                            Some((k, v))
                        } else {
                            None
                        }
                    })
                    .collect();
            }
            (Some(PlayerStatus::Alive), Some(SpecialRole::Doctor)) => {
                game_info.player_to_role = HashMap::from_iter([(client, SpecialRole::Doctor)]);
            }
            (Some(PlayerStatus::Alive), Some(SpecialRole::Detective)) => {
                game_info.player_to_role = HashMap::from_iter([(client, SpecialRole::Detective)]);
            }
            (Some(PlayerStatus::Alive), None) => {}
        }

        Some(game_info)
    }
}

/// Manages client connections, client requests, and active Mafia game state.
#[derive(Clone)]
pub struct MafiaGameServer(Arc<RwLock<MafiaGameServerInner>>);

impl MafiaGameServer {
    pub fn new(config: MafiaGameServerConfig) -> Self {
        MafiaGameServer(Arc::new(RwLock::new(MafiaGameServerInner {
            config,
            clients: ClientState::new(),
            active_game: None,
        })))
    }

    /// Returns `true` if the server has an active game that is not in a won condition.
    pub fn in_active_game(&self) -> bool {
        self.0.read().unwrap().in_active_game()
    }

    /// Starts a new game. Returns an `Err` if there is an active game.
    pub fn start_game<S: Rng>(&self, config: GameConfig, seed: S) -> Result<(), MafiaGameError> {
        let mut slf = self.0.write().unwrap();

        if slf.in_active_game() {
            return Err(MafiaGameError::GameInProgress);
        }

        slf.purge_disconnected_clients();

        let game = Game::start(config, &slf.clients, seed)?;
        slf.active_game = Some(game);

        for client in &slf.clients.all_client_ids() {
            slf.clients.send_event(
                std::iter::once(client).collect(),
                Event::SetGame(slf.get_game_info_for(client).expect("is active game")),
            );
        }

        Ok(())
    }

    /// Ends the current game, returning an `Err` if no game is active.
    pub fn end_game(&self) -> Result<(), MafiaGameError> {
        let mut slf = self.0.write().unwrap();
        if std::mem::take(&mut slf.active_game).is_none() {
            return Err(MafiaGameError::NoGameInProgress);
        }

        slf.send_event(Event::EndGame);

        Ok(())
    }

    /// Ticks the active game state.
    pub fn do_tick(&self) {
        let mut slf = self.0.write().unwrap();

        let events = if let Some(game) = slf.active_game.as_mut() {
            game.poll_end_cycle()
        } else {
            slf.purge_disconnected_clients();

            vec![]
        };

        for event in events {
            slf.send_event(event);
        }
    }

    /// Handles a client request to connect.
    pub fn connect_client(
        &self,
        client_name: &str,
    ) -> Result<(ClientId, SessionToken), MafiaGameError> {
        let mut slf = self.0.write().unwrap();

        let (client_id, session_token) = slf.clients.connect_client(client_name)?;

        let new_client_info = slf.clients.get_client(client_id)?.get_info().clone();

        let connected_clients = slf.clients.all_client_info();

        slf.send_event(Event::ClientConnected(new_client_info));
        slf.clients.send_event(
            std::iter::once(client_id).collect(),
            Event::SetServerInfo(ServerInfo {
                connected_clients,
                active_game: slf.get_game_info_for(client_id),
            }),
        );

        Ok((client_id, session_token))
    }

    /// Handles a client request to disconnect.
    pub fn disconnect_client(&self, session_token: SessionToken) -> Result<(), MafiaGameError> {
        let mut slf = self.0.write().unwrap();

        let client_id = slf.clients.auth_client(session_token)?;

        slf.disconnect_client(client_id)
    }

    /// Force disconnect a client. Intended as an admin API.
    pub fn force_disconnect_client(&self, client_id: ClientId) -> Result<(), MafiaGameError> {
        let mut slf = self.0.write().unwrap();
        slf.disconnect_client(client_id)
    }

    /// Send a message to all clients. Intended as an admin API.
    pub fn broadcast_message(&self, message: Box<str>) {
        let slf = self.0.read().unwrap();

        let event = Event::MessageReceived(Message {
            channel: EventChannel::Public,
            contents: message,
            from: Entity::System,
        });

        slf.send_event(event);
    }

    /// Handles a client request to send a message to other clients. Messages are routed according
    /// to the current game state.
    pub fn send_message(
        &self,
        session_token: SessionToken,
        message: Box<str>,
    ) -> Result<(), MafiaGameError> {
        let slf = self.0.read().unwrap();
        let client_id = slf.clients.auth_client(session_token)?;

        let channel = if let Ok(game) = slf.get_active_game() {
            if matches!(
                game.get_player_status(client_id),
                Some(PlayerStatus::Dead) | None
            ) {
                EventChannel::Spectator
            }
            // Player is alive
            else if game.get_cycle() == Cycle::Day {
                EventChannel::Public
            }
            // Is night
            else if game.get_player_allegiance(client_id) == Allegiance::Mafia {
                EventChannel::Mafia
            }
            // If villager sends a message at night, only spectators can see.
            else {
                EventChannel::Spectator
            }
        } else {
            EventChannel::Public
        };

        let event = Event::MessageReceived(Message {
            channel,
            contents: message,
            from: Entity::Client(client_id),
        });

        slf.send_event(event);

        Ok(())
    }

    /// Handles a client request to drain all event currently in the client's inbox.
    pub fn take_events(
        &self,
        session_token: SessionToken,
    ) -> Result<Box<[Arc<Event>]>, MafiaGameError> {
        let slf = self.0.write().unwrap();
        let client_id = slf.clients.auth_client(session_token)?;

        Ok(slf.clients.take_events(client_id))
    }

    /// Handles a client request to vote in a particular cycle. If `None` is passed, means the
    /// client is explicitly skipping this vote.
    pub fn cast_vote(
        &self,
        session_token: SessionToken,
        target: Option<ClientId>,
    ) -> Result<(), MafiaGameError> {
        let mut slf = self.0.write().unwrap();
        let client_id = slf.clients.auth_client(session_token)?;

        let game = slf.get_active_game_mut()?;

        game.cast_vote(client_id, target)?;

        let channel = if game.get_cycle() == Cycle::Day {
            EventChannel::Public
        }
        // Is night
        else if game.get_player_allegiance(client_id) == Allegiance::Mafia {
            EventChannel::Mafia
        }
        // Only self + spectator can see this vote.
        else {
            EventChannel::Spectator
        };

        let events = [Event::VoteIssued {
            voter: client_id,
            target,
            channel,
        }]
        .into_iter()
        .chain(game.poll_end_cycle());

        for event in events {
            slf.send_event(event);
        }

        Ok(())
    }

    /// Starts a new background thread for ticking the game state that can be stopped using
    /// [`TickerShutdown::do_shutdown`].
    pub fn start_server_ticker(&self, tick_rate: Duration) -> (TickerShutdown, JoinHandle<()>) {
        let server = self.clone();
        let shutdown = TickerShutdown::new();

        let handle = thread::spawn({
            let shutdown = shutdown.clone();

            move || {
                loop {
                    if shutdown.is_shutdown() {
                        return;
                    }

                    server.do_tick();

                    thread::sleep(tick_rate);
                }
            }
        });

        (shutdown, handle)
    }
}

#[derive(Clone)]
pub struct TickerShutdown(Arc<AtomicBool>);

impl TickerShutdown {
    fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }

    fn is_shutdown(&self) -> bool {
        self.0.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn do_shutdown(&self) {
        self.0.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    mod client;
    mod game;
    mod server;
}
