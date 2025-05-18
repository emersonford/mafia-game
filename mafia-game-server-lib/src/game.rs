//! Core logic for a game of Mafia.

use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Duration;
use std::time::SystemTime;

use mafia_game_lib::Allegiance;
use mafia_game_lib::ClientId;
use mafia_game_lib::Cycle;
use mafia_game_lib::PlayerStatus;
use mafia_game_lib::SpecialRole;
use rand::Rng;
use rand::seq::SliceRandom;
use tracing::field;

use crate::client::ClientState;
use crate::error::MafiaGameError;

// TODO(emersonford): allow this to be populated at runtime
#[derive(Clone, Debug)]
pub struct GameConfig {
    pub start_cycle: Cycle,
    pub time_for_day: Duration,
    /// End the day cycle early if all votes have been submitted.
    pub end_day_after_all_votes: bool,
    pub time_for_night: Duration,
    /// End the night cycle early if all votes have been submitted.
    pub end_night_after_all_votes: bool,
    pub num_special_roles: HashMap<SpecialRole, usize>,
    /// Time after cycle start during switch votes are rejected.
    ///
    /// Useful to avoid last-minute votes leaking into the next cycle and spoiling results.
    pub vote_grace_period: Duration,
    // TODO(emersonford): add option to reveal roles on death
}

/// State for an active game.
pub(crate) struct Game {
    config: GameConfig,
    role_to_players: HashMap<SpecialRole, Vec<ClientId>>,
    player_to_role: HashMap<ClientId, SpecialRole>,
    player_status: HashMap<ClientId, PlayerStatus>,
    cycle: Cycle,
    day_num: usize,
    cycle_start: SystemTime,
    /// Map of voter -> who they are voting for.
    ///
    /// If value is `None`, means the voter skipped voting.
    votes: HashMap<ClientId, Option<ClientId>>,
}

impl Game {
    pub(crate) fn start<S: Rng>(
        config: GameConfig,
        clients: &ClientState,
        mut seed: S,
    ) -> Result<Self, MafiaGameError> {
        let mut clients = clients.list_clients().values().copied().collect::<Vec<_>>();
        // Sort for determinism with deterministic seed.
        clients.sort();

        let num_mafia_roles = config
            .num_special_roles
            .get(&SpecialRole::Mafia)
            .copied()
            .unwrap_or(0);
        let total_special_roles = config.num_special_roles.values().copied().sum::<usize>();

        if num_mafia_roles == 0 {
            return Err(MafiaGameError::InvalidGameConfig(
                "need at least 1 mafia, got 0".to_string(),
            ));
        }

        if num_mafia_roles * 2 >= clients.len() {
            return Err(MafiaGameError::NotEnoughPlayers(format!(
                "need at least {} players to play with {} mafia, only have {} players",
                num_mafia_roles * 2 + 1,
                num_mafia_roles,
                clients.len()
            )));
        }

        if total_special_roles > clients.len() {
            return Err(MafiaGameError::NotEnoughPlayers(format!(
                "{} special roles were provided, but only have {} players",
                total_special_roles,
                clients.len()
            )));
        }

        let mut num_special_roles = config
            .num_special_roles
            .clone()
            .into_iter()
            .collect::<Vec<_>>();
        // Sort for determinism with deterministic seed.
        num_special_roles.sort_by_key(|(role, _)| *role);

        clients.shuffle(&mut seed);

        let mut role_to_players = HashMap::new();
        let mut player_to_role = HashMap::new();

        let mut client_idx = 0;
        for (special_role, num) in num_special_roles {
            for _ in 0..num {
                let client_id = clients[client_idx];

                role_to_players
                    .entry(special_role)
                    .or_insert_with(Vec::new)
                    .push(client_id);
                player_to_role.insert(client_id, special_role);

                client_idx += 1;
            }
        }

        let cycle = config.start_cycle;

        Ok(Game {
            config,
            role_to_players,
            player_to_role,
            player_status: clients
                .into_iter()
                .map(|client_id| (client_id, PlayerStatus::Alive))
                .collect(),
            cycle,
            day_num: 0,
            cycle_start: SystemTime::now(),
            votes: HashMap::new(),
        })
    }

    #[cfg(test)]
    pub(crate) fn get_player_roles(&self) -> &HashMap<ClientId, SpecialRole> {
        &self.player_to_role
    }

    pub(crate) fn get_player_status(&self, client_id: ClientId) -> PlayerStatus {
        self.player_status
            .get(&client_id)
            .copied()
            .unwrap_or(PlayerStatus::NotPlaying)
    }

    pub(crate) fn get_player_role(&self, client_id: ClientId) -> Option<SpecialRole> {
        self.player_to_role.get(&client_id).copied()
    }

    pub(crate) fn get_player_allegiance(&self, client_id: ClientId) -> Allegiance {
        self.get_player_role(client_id)
            .map_or(Allegiance::Villagers, |role| role.allegiance())
    }

    pub(crate) fn get_alive_players(&self) -> impl Iterator<Item = ClientId> {
        self.player_status.iter().filter_map(|(client_id, status)| {
            if *status == PlayerStatus::Alive {
                Some(*client_id)
            } else {
                None
            }
        })
    }

    pub(crate) fn get_dead_players<T: IntoIterator<Item = ClientId>>(
        &self,
        all_clients: T,
    ) -> impl Iterator<Item = ClientId> {
        all_clients
            .into_iter()
            .filter(|client_id| self.get_player_status(*client_id) != PlayerStatus::Alive)
    }

    fn end_cycle(&mut self) -> &mut Self {
        tracing::info!("ending cycle with votes: {:?}", self.votes);

        match self.cycle {
            Cycle::Day => {
                let num_votes_for_player =
                    self.votes
                        .iter()
                        .fold(HashMap::new(), |mut acc, (_, &target)| {
                            if let Some(target) = target {
                                *acc.entry(target).or_insert(0) += 1;
                            }

                            acc
                        });

                let num_players_alive = self.get_alive_players().count();

                if let Some((voted_player, _)) = num_votes_for_player
                    .into_iter()
                    .find(|(_, count)| count * 2 > num_players_alive)
                {
                    tracing::info!("{:?} was killed during the day", voted_player);
                    // TODO(emersonford): add event for vote result / death
                    *self
                        .player_status
                        .get_mut(&voted_player)
                        .expect("valid player") = PlayerStatus::Dead;
                }

                // TODO(emersonford): add event for failed vote result
            }
            Cycle::Night => {
                let protected_players = self.role_to_players.get(&SpecialRole::Doctor).map_or_else(
                    HashSet::new,
                    |players| {
                        players
                            .iter()
                            .filter_map(|client_id| self.votes.get(client_id))
                            .flatten()
                            .copied()
                            .collect::<HashSet<_>>()
                    },
                );

                let num_mafia_votes_for_player = self
                    .votes
                    .iter()
                    .filter(|(voter, _)| self.get_player_allegiance(**voter) == Allegiance::Mafia)
                    .fold(HashMap::new(), |mut acc, (_, &target)| {
                        if let Some(target) = target {
                            *acc.entry(target).or_insert(0) += 1;
                        }
                        acc
                    });

                let num_mafia_alive = self
                    .get_alive_players()
                    .filter(|client_id| self.get_player_allegiance(*client_id) == Allegiance::Mafia)
                    .count();

                if let Some((mafia_voted_player, _)) = num_mafia_votes_for_player
                    .into_iter()
                    .find(|(_, count)| count * 2 > num_mafia_alive)
                {
                    // TODO(emersonford): add event for vote result / death
                    if !protected_players.contains(&mafia_voted_player) {
                        tracing::info!(
                            "{:?} was killed by the mafia in the night",
                            mafia_voted_player
                        );
                        *self
                            .player_status
                            .get_mut(&mafia_voted_player)
                            .expect("valid player") = PlayerStatus::Dead;
                    } else {
                        tracing::info!(
                            "{:?} was protected from a mafia kill in the night",
                            mafia_voted_player
                        );
                    }
                }
                // TODO(emersonford): add event for failed vote result

                for investigator in self
                    .role_to_players
                    .get(&SpecialRole::Detective)
                    .into_iter()
                    .flatten()
                {
                    if let Some(target) = self.votes.get(investigator).copied().flatten() {
                        let allegiance = self.get_player_allegiance(target);

                        tracing::info!(
                            "{:?} was investigated by {:?} and discovered to be {:?}",
                            target,
                            investigator,
                            allegiance
                        );

                        // TODO(emersonford): add event for investigation
                    }
                }
            }
            _ => {
                return self;
            }
        }

        let num_mafia_alive = self
            .get_alive_players()
            .filter(|client_id| self.get_player_allegiance(*client_id) == Allegiance::Mafia)
            .count();

        if num_mafia_alive == 0 {
            tracing::info!("all mafia eliminated, villagers win");
            // TODO(emersonford): add event for villager win
            self.cycle = Cycle::Won(Allegiance::Villagers);
            return self;
        }

        let num_players_alive = self.get_alive_players().count();

        if num_mafia_alive * 2 >= num_players_alive {
            tracing::info!("#mafia >= #non mafia; mafia win");
            // TODO(emersonford): add event for mafia win
            self.cycle = Cycle::Won(Allegiance::Mafia);
            return self;
        }

        if self.day_num >= 100 {
            tracing::error!("game exceeded 100 rounds, defaulting win to mafia");
            // TODO(emersonford): add event for mafia win
            self.cycle = Cycle::Won(Allegiance::Mafia);
            return self;
        }

        self.votes = HashMap::new();
        self.cycle = self.cycle.next();
        self.day_num = if matches!(self.cycle, Cycle::Day) {
            self.day_num + 1
        } else {
            self.day_num
        };

        self
    }

    #[tracing::instrument(
        skip_all,
        fields(
            cycle = format!("{:?} {}", self.cycle, self.day_num),
            voter = voter.0,
            target = field::debug(target.map(|v| v.0)),
        )
    )]
    pub(crate) fn cast_vote(
        &mut self,
        voter: ClientId,
        target: Option<ClientId>,
    ) -> Result<&mut Self, MafiaGameError> {
        if self.get_player_status(voter) != PlayerStatus::Alive {
            return Err(MafiaGameError::InvalidVote(format!(
                "voter {:?} is not alive",
                voter
            )));
        }

        if target.is_some_and(|t| self.get_player_status(t) != PlayerStatus::Alive) {
            return Err(MafiaGameError::InvalidVote(format!(
                "target for vote {:?} is not alive",
                target
            )));
        }

        if SystemTime::now()
            .duration_since(self.cycle_start)
            .unwrap_or(Duration::from_secs(0))
            < self.config.vote_grace_period
        {
            return Err(MafiaGameError::InvalidVote(format!(
                "must wait {:?} after cycle start to cast vote",
                self.config.vote_grace_period
            )));
        }

        match self.cycle {
            Cycle::Day => {
                // TODO(emersonford): add event for vote cast
                self.votes.insert(voter, target);
            }
            Cycle::Night => {
                if !self.player_to_role.get(&voter).is_some_and(|v| {
                    matches!(
                        v,
                        SpecialRole::Mafia | SpecialRole::Doctor | SpecialRole::Detective
                    )
                }) {
                    return Err(MafiaGameError::InvalidVote(format!(
                        "{:?} does not have a role eligible to vote in {:?}",
                        voter, self.cycle
                    )));
                }

                // TODO(emersonford): add event for vote cast
                self.votes.insert(voter, target);
            }
            Cycle::Won(_) => {
                return Err(MafiaGameError::InvalidVote("game is complete".to_string()));
            }
        }

        Ok(self.poll_end_cycle())
    }

    /// Checks if we've met the conditions to end the cycle, and if so, ends the cycle.
    #[tracing::instrument(
        skip(self),
        fields(cycle = format!("{:?} {}", self.cycle, self.day_num)),
    )]
    pub(crate) fn poll_end_cycle(&mut self) -> &mut Self {
        if matches!(self.cycle, Cycle::Won(_)) {
            return self;
        }

        if SystemTime::now()
            .duration_since(self.cycle_start)
            .unwrap_or(Duration::from_secs(0))
            > if self.cycle == Cycle::Day {
                self.config.time_for_day
            } else {
                self.config.time_for_night
            }
        {
            tracing::info!("reached cycle end time, ending cycle");
            return self.end_cycle();
        }

        if self.cycle == Cycle::Day && self.config.end_day_after_all_votes {
            let num_players_alive = self.get_alive_players().count();

            if self.votes.len() == num_players_alive {
                tracing::info!("all votes cast, ending cycle");
                return self.end_cycle();
            }
        }

        if self.cycle == Cycle::Night && self.config.end_night_after_all_votes {
            let num_special_roles_alive = self
                .get_alive_players()
                .filter(|client_id| self.player_to_role.contains_key(client_id))
                .count();

            if self.votes.len() == num_special_roles_alive {
                tracing::info!("all votes cast, ending cycle");
                return self.end_cycle();
            }
        }

        self
    }

    pub(crate) fn get_cycle(&self) -> Cycle {
        self.cycle
    }
}
