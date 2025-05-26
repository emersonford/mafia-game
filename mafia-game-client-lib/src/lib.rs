use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::MutexGuard;

use mafia_game_lib::Allegiance;
use mafia_game_lib::ClientId;
use mafia_game_lib::Message;
use mafia_game_lib::PlayerStatus;
use mafia_game_lib::ServerInfo;
use mafia_game_lib::SessionToken;
use mafia_game_lib::SpecialRole;

pub const MAX_MESSAGES_HISTORY: usize = 200;

/// Identity information for the client connection.
pub struct MafiaClientIdent {
    pub id: ClientId,
    pub session_token: SessionToken,
}

pub struct MafiaClientInner {
    pub server_info: ServerInfo,
    pub messages: VecDeque<Message>,
}

/// Maintains client-side state about a mafia game and handles drawing to the terminal.
pub struct MafiaClient {
    ident: MafiaClientIdent,
    inner: Mutex<MafiaClientInner>,
}

impl MafiaClient {
    pub fn new(ident: MafiaClientIdent, server_info: ServerInfo) -> Self {
        Self {
            ident,
            inner: Mutex::new(MafiaClientInner {
                server_info,
                messages: VecDeque::with_capacity(MAX_MESSAGES_HISTORY),
            }),
        }
    }

    pub fn get_ident(&self) -> &MafiaClientIdent {
        &self.ident
    }

    pub fn get_inner<'a>(&'a self) -> MutexGuard<'a, MafiaClientInner> {
        self.inner.lock().unwrap()
    }

    pub fn apply_event(&self, event: mafia_game_lib::Event) {
        let mut lock = self.inner.lock().unwrap();

        // TODO(emersonford): translate these into messages

        match event {
            mafia_game_lib::Event::SetServerInfo(new_info) => {
                lock.server_info = new_info;
            }
            mafia_game_lib::Event::SetGame(new_game) => {
                lock.server_info.active_game = Some(new_game);
            }
            mafia_game_lib::Event::EndGame => {
                lock.server_info.active_game = None;
            }
            mafia_game_lib::Event::ClientConnected(client_info) => {
                lock.server_info
                    .connected_clients
                    .insert(client_info.id, client_info);
            }
            mafia_game_lib::Event::ClientDisconnected(client_id) => {
                lock.server_info.connected_clients.remove(&client_id);
            }
            mafia_game_lib::Event::MessageReceived(message) => {
                if lock.messages.len() >= MAX_MESSAGES_HISTORY {
                    lock.messages.pop_front();
                }

                lock.messages.push_back(message);
            }
            mafia_game_lib::Event::VoteIssued {
                voter,
                target,
                channel: _,
            } => {
                if let Some(game) = &mut lock.server_info.active_game {
                    game.votes.insert(voter, target);
                }
            }
            mafia_game_lib::Event::FailedVote {
                cycle: _,
                channel: _,
            } => {}
            mafia_game_lib::Event::SetCycle {
                start_time_unix_ts_secs,
                duration_secs,
                cycle,
                day_num,
            } => {
                if let Some(game) = &mut lock.server_info.active_game {
                    game.current_cycle = cycle;
                    game.day_num = day_num;
                    game.cycle_start_time_unix_ts_secs = start_time_unix_ts_secs;
                    game.cycle_duration_secs = duration_secs;
                    game.votes = HashMap::new();
                }
            }
            mafia_game_lib::Event::PlayerKilled {
                player,
                cycle: _,
                death_message: _,
            } => {
                if let Some(game) = &mut lock.server_info.active_game {
                    game.player_status.entry(player).and_modify(|e| {
                        *e = PlayerStatus::Dead;
                    });
                }
            }
            mafia_game_lib::Event::PlayerInvestigated {
                actor: _,
                target,
                allegiance,
            } => {
                if let Some(game) = &mut lock.server_info.active_game {
                    if allegiance == Allegiance::Mafia {
                        game.player_to_role.insert(target, SpecialRole::Mafia);
                    }
                }
            }
            mafia_game_lib::Event::GameWon {
                player_to_role,
                side,
            } => {
                if let Some(game) = &mut lock.server_info.active_game {
                    game.player_to_role = player_to_role;
                    game.winner = Some(side);
                }
            }
        }
    }
}
