//! Provides an implementation for the [Mafia game](https://en.wikipedia.org/wiki/Mafia_(party_game)).

use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use client::ClientId;
use client::ClientState;
use client::Entity;
use client::Message;
use client::MessageChannel;
use client::SessionToken;
use game::Allegiance;
use game::Cycle;
use game::Game;
use game::GameConfig;
use game::PlayerStatus;
use rand::Rng;

pub mod client;
mod consts;
mod error;
mod event;
pub mod game;

pub use error::MafiaGameError;

pub struct MafiaGameServerConfig {
    /// Max time a client can be inactive before we force disconnect it.
    pub max_client_inactive_time: Duration,
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

        if matches!(game.get_cycle(), Cycle::Won(_)) {
            return Err(MafiaGameError::NoGameInProgress);
        }

        Ok(game)
    }

    fn get_active_game_mut(&mut self) -> Result<&mut Game, MafiaGameError> {
        let Some(game) = self.active_game.as_mut() else {
            return Err(MafiaGameError::NoGameInProgress);
        };

        if matches!(game.get_cycle(), Cycle::Won(_)) {
            return Err(MafiaGameError::NoGameInProgress);
        }

        Ok(game)
    }

    fn in_active_game(&self) -> bool {
        self.active_game
            .as_ref()
            .is_some_and(|game| !matches!(game.get_cycle(), Cycle::Won(_)))
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

        let max_client_inactive_time = slf.config.max_client_inactive_time;

        slf.clients
            .purge_disconnected_clients(max_client_inactive_time);

        let game = Game::start(config, &slf.clients, seed)?;
        slf.active_game = Some(game);

        Ok(())
    }

    /// Ends the current game, returning an `Err` if no game is active.
    pub fn end_game(&self) -> Result<(), MafiaGameError> {
        if std::mem::take(&mut self.0.write().unwrap().active_game).is_none() {
            return Err(MafiaGameError::NoGameInProgress);
        }

        Ok(())
    }

    /// Ticks the active game state.
    pub fn do_tick(&self) {
        let mut slf = self.0.write().unwrap();

        if let Some(game) = slf.active_game.as_mut() {
            game.poll_end_cycle();
        } else {
            let max_client_inactive_time = slf.config.max_client_inactive_time;

            slf.clients
                .purge_disconnected_clients(max_client_inactive_time);
        }
    }

    /// Handles a client request to connect.
    pub fn connect_client(
        &self,
        client_name: &str,
    ) -> Result<(ClientId, SessionToken), MafiaGameError> {
        let mut slf = self.0.write().unwrap();

        slf.clients.connect_client(client_name)
    }

    /// Handles a client request to disconnect.
    pub fn disconnect_client(&self, session_token: SessionToken) -> Result<(), MafiaGameError> {
        let slf = self.0.read().unwrap();

        let client_id = slf.clients.auth_client(session_token)?;

        slf.clients.disconnect_client(client_id)
    }

    /// Force disconnect a client. Intended as an admin API.
    pub fn force_disconnect_client(&self, client_id: ClientId) -> Result<(), MafiaGameError> {
        let slf = self.0.read().unwrap();

        slf.clients.disconnect_client(client_id)
    }

    /// Send a message to all clients. Intended as an admin API.
    pub fn broadcast_message(&self, message: Box<str>) {
        let slf = self.0.read().unwrap();

        let message = Message {
            channel: MessageChannel::Public,
            contents: message,
            from: Entity::System,
        };

        let all_clients: Box<[ClientId]> = slf.clients.list_clients().values().copied().collect();

        slf.clients.send_message(&all_clients, message);
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

        let all_clients: Box<[ClientId]> = slf.clients.list_clients().values().copied().collect();

        let (channel, to_clients): (MessageChannel, Box<[ClientId]>) =
            if let Ok(game) = slf.get_active_game() {
                let alive_clients = game.get_alive_players();
                let dead_clients = game.get_dead_players(all_clients);

                // Dead talks to the the dead.
                if matches!(
                    game.get_player_status(client_id),
                    PlayerStatus::Dead | PlayerStatus::NotPlaying
                ) {
                    (MessageChannel::Spectator, dead_clients.collect())
                }
                // Day: alive talk to both the alive and the dead.
                else if game.get_cycle() == Cycle::Day {
                    (
                        MessageChannel::Public,
                        alive_clients.chain(dead_clients).collect(),
                    )
                }
                // Night: alive mafia talk to aliva mafia and the dead.
                else if game.get_player_allegiance(client_id) == Allegiance::Mafia {
                    (
                        MessageChannel::Mafia,
                        alive_clients
                            .filter(|&id| game.get_player_allegiance(id) == Allegiance::Mafia)
                            .chain(dead_clients)
                            .collect(),
                    )
                }
                // Night: sleeping villagers talk to self and the dead.
                else {
                    (
                        MessageChannel::Spectator,
                        dead_clients.chain(std::iter::once(client_id)).collect(),
                    )
                }
            } else {
                (MessageChannel::Public, all_clients)
            };

        let message = Message {
            channel,
            contents: message,
            from: Entity::Client(client_id),
        };

        slf.clients.send_message(&to_clients, message);

        Ok(())
    }

    /// Handles a client request to drain all messages currently in the client's inbox.
    pub fn take_messages(
        &self,
        session_token: SessionToken,
    ) -> Result<Box<[Arc<Message>]>, MafiaGameError> {
        let slf = self.0.write().unwrap();
        let client_id = slf.clients.auth_client(session_token)?;

        Ok(slf.clients.take_messages(client_id))
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

        slf.get_active_game_mut()?.cast_vote(client_id, target)?;

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
