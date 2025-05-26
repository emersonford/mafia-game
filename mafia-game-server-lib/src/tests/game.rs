use std::collections::HashMap;
use std::time::Duration;

use mafia_game_lib::Event;
use mafia_game_lib::EventChannel;
use rand::rngs::mock::StepRng;

use crate::Game;
use crate::client::ClientState;
use crate::consts::DAY_DEATH_MESSAGES;
use crate::consts::NIGHT_DEATH_MESSAGES;
use crate::error::MafiaGameError;
use crate::game::GameConfig;
use crate::game::is_alive;
use mafia_game_lib::Allegiance;
use mafia_game_lib::Cycle;
use mafia_game_lib::SpecialRole;

#[test]
fn test_game_validation() {
    let mut client_state = ClientState::new();

    client_state.connect_client("garnet").unwrap();
    client_state.connect_client("amethyst").unwrap();
    client_state.connect_client("pearl").unwrap();

    assert!(matches!(
        Game::start(
            GameConfig {
                start_cycle: Cycle::Day,
                time_for_day: Duration::from_secs(0),
                end_day_after_all_votes: true,
                time_for_night: Duration::from_secs(0),
                end_night_after_all_votes: true,
                num_special_roles: HashMap::new(),
                vote_grace_period: Duration::from_secs(0)
            },
            &client_state,
            StepRng::new(1, 1)
        ),
        Err(MafiaGameError::InvalidGameConfig(_))
    ));

    assert!(matches!(
        Game::start(
            GameConfig {
                start_cycle: Cycle::Day,
                time_for_day: Duration::from_secs(0),
                end_day_after_all_votes: true,
                time_for_night: Duration::from_secs(0),
                end_night_after_all_votes: true,
                num_special_roles: HashMap::from_iter([(SpecialRole::Mafia, 2)]),
                vote_grace_period: Duration::from_secs(0)
            },
            &client_state,
            StepRng::new(1, 1)
        ),
        Err(MafiaGameError::NotEnoughPlayers(_))
    ));

    assert!(matches!(
        Game::start(
            GameConfig {
                start_cycle: Cycle::Day,
                time_for_day: Duration::from_secs(0),
                end_day_after_all_votes: true,
                time_for_night: Duration::from_secs(0),
                end_night_after_all_votes: true,
                num_special_roles: HashMap::from_iter([
                    (SpecialRole::Mafia, 1),
                    (SpecialRole::Detective, 3)
                ]),
                vote_grace_period: Duration::from_secs(0)
            },
            &client_state,
            StepRng::new(1, 1)
        ),
        Err(MafiaGameError::NotEnoughPlayers(_))
    ));

    assert!(matches!(
        Game::start(
            GameConfig {
                start_cycle: Cycle::Day,
                time_for_day: Duration::from_secs(0),
                end_day_after_all_votes: true,
                time_for_night: Duration::from_secs(0),
                end_night_after_all_votes: true,
                num_special_roles: HashMap::from_iter([(SpecialRole::Mafia, 1)]),
                vote_grace_period: Duration::from_secs(0)
            },
            &client_state,
            StepRng::new(1, 1)
        ),
        Ok(_)
    ));

    assert!(matches!(
        Game::start(
            GameConfig {
                start_cycle: Cycle::Day,
                time_for_day: Duration::from_secs(0),
                end_day_after_all_votes: true,
                time_for_night: Duration::from_secs(0),
                end_night_after_all_votes: true,
                num_special_roles: HashMap::from_iter([
                    (SpecialRole::Mafia, 1),
                    (SpecialRole::Detective, 1)
                ]),
                vote_grace_period: Duration::from_secs(0)
            },
            &client_state,
            StepRng::new(1, 1)
        ),
        Ok(_)
    ));

    assert!(matches!(
        Game::start(
            GameConfig {
                start_cycle: Cycle::Day,
                time_for_day: Duration::from_secs(0),
                end_day_after_all_votes: true,
                time_for_night: Duration::from_secs(0),
                end_night_after_all_votes: true,
                num_special_roles: HashMap::from_iter([
                    (SpecialRole::Mafia, 1),
                    (SpecialRole::Detective, 1),
                    (SpecialRole::Doctor, 1)
                ]),
                vote_grace_period: Duration::from_secs(0)
            },
            &client_state,
            StepRng::new(1, 1)
        ),
        Ok(_)
    ));
}

#[test_log::test]
fn test_game_single_cycle_day() {
    let mut client_state = ClientState::new();

    let (client1_id, _) = client_state.connect_client("garnet").unwrap();
    let (client2_id, _) = client_state.connect_client("amethyst").unwrap();
    let (client3_id, _) = client_state.connect_client("pearl").unwrap();

    let mut game = Game::start(
        GameConfig {
            start_cycle: Cycle::Day,
            time_for_day: Duration::from_secs(10),
            end_day_after_all_votes: true,
            time_for_night: Duration::from_secs(10),
            end_night_after_all_votes: true,
            num_special_roles: HashMap::from_iter([(SpecialRole::Mafia, 1)]),
            vote_grace_period: Duration::from_secs(0),
        },
        &client_state,
        StepRng::new(1, 1),
    )
    .unwrap();

    assert_eq!(
        *game.get_player_roles(),
        HashMap::from_iter([(client3_id, SpecialRole::Mafia)]),
    );

    game.cast_vote(client1_id, Some(client3_id))
        .unwrap()
        .cast_vote(client2_id, Some(client3_id))
        .unwrap()
        .cast_vote(client3_id, None)
        .unwrap();

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client3_id,
                cycle: Cycle::Day,
                death_message: Box::from(DAY_DEATH_MESSAGES[0])
            },
            Event::GameWon {
                player_to_role: HashMap::from_iter([(client3_id, SpecialRole::Mafia)]),
                side: Allegiance::Villagers
            }
        ]
    );

    assert_eq!(game.get_winner(), Some(Allegiance::Villagers));
}

#[test_log::test]
fn test_game_single_cycle_night() {
    let mut client_state = ClientState::new();

    let (client1_id, _) = client_state.connect_client("garnet").unwrap();
    let (_client2_id, _) = client_state.connect_client("amethyst").unwrap();
    let (client3_id, _) = client_state.connect_client("pearl").unwrap();

    let mut game = Game::start(
        GameConfig {
            start_cycle: Cycle::Night,
            time_for_day: Duration::from_secs(10),
            end_day_after_all_votes: true,
            time_for_night: Duration::from_secs(10),
            end_night_after_all_votes: true,
            num_special_roles: HashMap::from_iter([(SpecialRole::Mafia, 1)]),
            vote_grace_period: Duration::from_secs(0),
        },
        &client_state,
        StepRng::new(1, 1),
    )
    .unwrap();

    assert_eq!(
        *game.get_player_roles(),
        HashMap::from_iter([(client3_id, SpecialRole::Mafia)]),
    );

    game.cast_vote(client3_id, Some(client1_id)).unwrap();

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client1_id,
                cycle: Cycle::Night,
                death_message: Box::from(NIGHT_DEATH_MESSAGES[0])
            },
            Event::GameWon {
                player_to_role: HashMap::from_iter([(client3_id, SpecialRole::Mafia)]),
                side: Allegiance::Mafia
            }
        ]
    );

    assert_eq!(game.get_winner(), Some(Allegiance::Mafia));
}

#[test_log::test]
fn test_game_vote_rejections_day() {
    let mut client_state = ClientState::new();

    let (client1_id, _) = client_state.connect_client("garnet").unwrap();
    let (client2_id, _) = client_state.connect_client("amethyst").unwrap();
    let (client3_id, _) = client_state.connect_client("pearl").unwrap();
    let (client4_id, _) = client_state.connect_client("steven").unwrap();
    let (client5_id, _) = client_state.connect_client("connie").unwrap();
    let (client6_id, _) = client_state.connect_client("pink").unwrap();
    let (client7_id, _) = client_state.connect_client("blue").unwrap();

    let mut game = Game::start(
        GameConfig {
            start_cycle: Cycle::Day,
            time_for_day: Duration::from_secs(10),
            end_day_after_all_votes: true,
            time_for_night: Duration::from_secs(10),
            end_night_after_all_votes: true,
            num_special_roles: HashMap::from_iter([
                (SpecialRole::Mafia, 2),
                (SpecialRole::Detective, 1),
                (SpecialRole::Doctor, 1),
            ]),
            vote_grace_period: Duration::from_secs(0),
        },
        &client_state,
        StepRng::new(1, 1),
    )
    .unwrap();

    // Joined after the game started.
    let (client8_id, _) = client_state.connect_client("yellow").unwrap();

    assert_eq!(
        *game.get_player_roles(),
        HashMap::from_iter([
            (client7_id, SpecialRole::Mafia),
            (client1_id, SpecialRole::Mafia),
            (client2_id, SpecialRole::Doctor),
            (client3_id, SpecialRole::Detective)
        ]),
    );

    // -- DAY 1 --
    // Everyone alive can vote during the day.
    for &client_id in client_state.list_clients().values() {
        if client_id == client8_id {
            assert!(matches!(
                game.cast_vote(client_id, None),
                Err(MafiaGameError::InvalidVote(_))
            ));
        } else {
            // Votes to invalid player fail.
            assert!(matches!(
                game.cast_vote(client_id, Some(client8_id)),
                Err(MafiaGameError::InvalidVote(_))
            ));

            game.cast_vote(client_id, None).unwrap();
        }
    }

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::FailedVote {
                cycle: Cycle::Day,
                channel: EventChannel::Public,
            },
            Event::SetCycle {
                cycle: Cycle::Night,
                day_num: 1,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- NIGHT 1 --
    // Only Mafia, Detective, and Doctor can vote during the night.
    for &client_id in client_state.list_clients().values() {
        if game.get_player_role(client_id).is_none() {
            assert!(matches!(
                game.cast_vote(client_id, None),
                Err(MafiaGameError::InvalidVote(_))
            ));
        } else {
            // Votes to invalid player fail.
            assert!(matches!(
                game.cast_vote(client_id, Some(client8_id)),
                Err(MafiaGameError::InvalidVote(_))
            ));
        }
    }

    game.cast_vote(client7_id, Some(client4_id)).unwrap();
    game.cast_vote(client1_id, Some(client4_id)).unwrap();
    game.cast_vote(client2_id, Some(client5_id)).unwrap();
    game.cast_vote(client3_id, Some(client5_id)).unwrap();

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client4_id,
                cycle: Cycle::Night,
                death_message: Box::from(NIGHT_DEATH_MESSAGES[0])
            },
            Event::PlayerInvestigated {
                actor: client3_id,
                target: client5_id,
                allegiance: Allegiance::Villagers,
            },
            Event::SetCycle {
                cycle: Cycle::Day,
                day_num: 2,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- DAY 2 --
    // Everyone but the dead player can vote.
    for &client_id in client_state.list_clients().values() {
        if client_id == client4_id || client_id == client8_id {
            assert!(matches!(
                game.cast_vote(client_id, None),
                Err(MafiaGameError::InvalidVote(_))
            ));
        } else {
            // Votes to dead player fail.
            assert!(matches!(
                game.cast_vote(client_id, Some(client4_id)),
                Err(MafiaGameError::InvalidVote(_))
            ));

            game.cast_vote(client_id, Some(client7_id)).unwrap();
        }
    }

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client7_id,
                cycle: Cycle::Day,
                death_message: Box::from(DAY_DEATH_MESSAGES[0])
            },
            Event::SetCycle {
                cycle: Cycle::Night,
                day_num: 2,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- NIGHT 2 --
    // Only alive Mafia, Detective, and Doctor can vote during the night.
    for &client_id in client_state.list_clients().values() {
        if game.get_player_role(client_id).is_none() {
            assert!(matches!(
                game.cast_vote(client_id, None),
                Err(MafiaGameError::InvalidVote(_))
            ));
        } else {
            // Votes to invalid player fail.
            assert!(matches!(
                game.cast_vote(client_id, Some(client4_id)),
                Err(MafiaGameError::InvalidVote(_))
            ));
        }
    }

    // Dead mafia can't vote.
    assert!(matches!(
        game.cast_vote(client7_id, Some(client5_id)),
        Err(MafiaGameError::InvalidVote(_))
    ));
    game.cast_vote(client1_id, Some(client5_id)).unwrap();
    game.cast_vote(client2_id, Some(client6_id)).unwrap();
    game.cast_vote(client3_id, Some(client6_id)).unwrap();

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client5_id,
                cycle: Cycle::Night,
                death_message: Box::from(NIGHT_DEATH_MESSAGES[0])
            },
            Event::PlayerInvestigated {
                actor: client3_id,
                target: client6_id,
                allegiance: Allegiance::Villagers,
            },
            Event::SetCycle {
                cycle: Cycle::Day,
                day_num: 3,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- DAY 3 --
    // Everyone but the dead players can vote.
    for &client_id in client_state.list_clients().values() {
        if client_id == client4_id
            || client_id == client5_id
            || client_id == client7_id
            || client_id == client8_id
        {
            assert!(matches!(
                game.cast_vote(client_id, None),
                Err(MafiaGameError::InvalidVote(_))
            ));
        } else {
            // Votes to dead player fail.
            assert!(matches!(
                game.cast_vote(client_id, Some(client5_id)),
                Err(MafiaGameError::InvalidVote(_))
            ));

            game.cast_vote(client_id, Some(client1_id)).unwrap();
        }
    }

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client1_id,
                cycle: Cycle::Day,
                death_message: Box::from(DAY_DEATH_MESSAGES[0])
            },
            Event::GameWon {
                player_to_role: HashMap::from_iter([
                    (client7_id, SpecialRole::Mafia),
                    (client1_id, SpecialRole::Mafia),
                    (client2_id, SpecialRole::Doctor),
                    (client3_id, SpecialRole::Detective)
                ]),
                side: Allegiance::Villagers
            }
        ]
    );

    // -- VILLAGERS WIN --
    // All votes fail.
    assert_eq!(game.get_winner(), Some(Allegiance::Villagers));

    for &client_id in client_state.list_clients().values() {
        assert!(matches!(
            game.cast_vote(client_id, None),
            Err(MafiaGameError::InvalidVote(_))
        ));
    }
}

#[test_log::test]
fn test_game_e2e_mafia_win() {
    let mut client_state = ClientState::new();

    let (client1_id, _) = client_state.connect_client("garnet").unwrap();
    let (client2_id, _) = client_state.connect_client("amethyst").unwrap();
    let (client3_id, _) = client_state.connect_client("pearl").unwrap();
    let (client4_id, _) = client_state.connect_client("steven").unwrap();
    let (client5_id, _) = client_state.connect_client("connie").unwrap();
    let (client6_id, _) = client_state.connect_client("pink").unwrap();
    let (client7_id, _) = client_state.connect_client("blue").unwrap();

    let mut game = Game::start(
        GameConfig {
            start_cycle: Cycle::Day,
            time_for_day: Duration::from_secs(10),
            end_day_after_all_votes: true,
            time_for_night: Duration::from_secs(10),
            end_night_after_all_votes: true,
            num_special_roles: HashMap::from_iter([
                (SpecialRole::Mafia, 2),
                (SpecialRole::Detective, 1),
                (SpecialRole::Doctor, 1),
            ]),
            vote_grace_period: Duration::from_secs(0),
        },
        &client_state,
        StepRng::new(1, 1),
    )
    .unwrap();

    // Joined after the game started.
    let (_client8_id, _) = client_state.connect_client("yellow").unwrap();

    assert_eq!(
        *game.get_player_roles(),
        HashMap::from_iter([
            (client7_id, SpecialRole::Mafia),
            (client1_id, SpecialRole::Mafia),
            (client2_id, SpecialRole::Doctor),
            (client3_id, SpecialRole::Detective)
        ]),
    );

    // -- DAY 1 --
    for client_id in &game.get_players(is_alive) {
        game.cast_vote(client_id, None).unwrap();
    }

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::FailedVote {
                cycle: Cycle::Day,
                channel: EventChannel::Public,
            },
            Event::SetCycle {
                cycle: Cycle::Night,
                day_num: 1,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- NIGHT 1 --
    game.cast_vote(client7_id, Some(client4_id)).unwrap();
    game.cast_vote(client1_id, Some(client4_id)).unwrap();
    game.cast_vote(client2_id, None).unwrap();
    game.cast_vote(client3_id, None).unwrap();

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client4_id,
                cycle: Cycle::Night,
                death_message: Box::from(NIGHT_DEATH_MESSAGES[0])
            },
            Event::SetCycle {
                cycle: Cycle::Day,
                day_num: 2,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- DAY 2 --
    for client_id in &game.get_players(is_alive) {
        game.cast_vote(client_id, Some(client5_id)).unwrap();
    }

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client5_id,
                cycle: Cycle::Day,
                death_message: Box::from(DAY_DEATH_MESSAGES[0])
            },
            Event::SetCycle {
                cycle: Cycle::Night,
                day_num: 2,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- NIGHT 2 --
    game.cast_vote(client7_id, Some(client6_id)).unwrap();
    game.cast_vote(client1_id, Some(client6_id)).unwrap();
    game.cast_vote(client2_id, None).unwrap();
    game.cast_vote(client3_id, None).unwrap();

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client6_id,
                cycle: Cycle::Night,
                death_message: Box::from(NIGHT_DEATH_MESSAGES[0])
            },
            Event::GameWon {
                player_to_role: HashMap::from_iter([
                    (client7_id, SpecialRole::Mafia),
                    (client1_id, SpecialRole::Mafia),
                    (client2_id, SpecialRole::Doctor),
                    (client3_id, SpecialRole::Detective)
                ]),
                side: Allegiance::Mafia
            }
        ]
    );

    // -- MAFIA WIN --
    assert_eq!(game.get_winner(), Some(Allegiance::Mafia));
}

#[test_log::test]
fn test_game_e2e_villagers_win() {
    let mut client_state = ClientState::new();

    let (client1_id, _) = client_state.connect_client("garnet").unwrap();
    let (client2_id, _) = client_state.connect_client("amethyst").unwrap();
    let (client3_id, _) = client_state.connect_client("pearl").unwrap();
    let (client4_id, _) = client_state.connect_client("steven").unwrap();
    let (_client5_id, _) = client_state.connect_client("connie").unwrap();
    let (client6_id, _) = client_state.connect_client("pink").unwrap();
    let (client7_id, _) = client_state.connect_client("blue").unwrap();

    let mut game = Game::start(
        GameConfig {
            start_cycle: Cycle::Day,
            time_for_day: Duration::from_secs(10),
            end_day_after_all_votes: true,
            time_for_night: Duration::from_secs(10),
            end_night_after_all_votes: true,
            num_special_roles: HashMap::from_iter([
                (SpecialRole::Mafia, 2),
                (SpecialRole::Detective, 1),
                (SpecialRole::Doctor, 1),
            ]),
            vote_grace_period: Duration::from_secs(0),
        },
        &client_state,
        StepRng::new(1, 1),
    )
    .unwrap();

    // Joined after the game started.
    let (_client8_id, _) = client_state.connect_client("yellow").unwrap();

    assert_eq!(
        *game.get_player_roles(),
        HashMap::from_iter([
            (client7_id, SpecialRole::Mafia),
            (client1_id, SpecialRole::Mafia),
            (client2_id, SpecialRole::Doctor),
            (client3_id, SpecialRole::Detective)
        ]),
    );

    // -- DAY 1 --
    for client_id in &game.get_players(is_alive) {
        game.cast_vote(client_id, None).unwrap();
    }

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::FailedVote {
                cycle: Cycle::Day,
                channel: EventChannel::Public,
            },
            Event::SetCycle {
                cycle: Cycle::Night,
                day_num: 1,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- NIGHT 1 --
    game.cast_vote(client7_id, Some(client4_id)).unwrap();
    game.cast_vote(client1_id, Some(client4_id)).unwrap();
    game.cast_vote(client2_id, None).unwrap();
    game.cast_vote(client3_id, None).unwrap();

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client4_id,
                cycle: Cycle::Night,
                death_message: Box::from(NIGHT_DEATH_MESSAGES[0])
            },
            Event::SetCycle {
                cycle: Cycle::Day,
                day_num: 2,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- DAY 2 --
    for client_id in &game.get_players(is_alive) {
        game.cast_vote(client_id, Some(client7_id)).unwrap();
    }

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client7_id,
                cycle: Cycle::Day,
                death_message: Box::from(DAY_DEATH_MESSAGES[0])
            },
            Event::SetCycle {
                cycle: Cycle::Night,
                day_num: 2,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- NIGHT 2 --
    game.cast_vote(client1_id, Some(client6_id)).unwrap();
    game.cast_vote(client2_id, None).unwrap();
    game.cast_vote(client3_id, None).unwrap();

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client6_id,
                cycle: Cycle::Night,
                death_message: Box::from(NIGHT_DEATH_MESSAGES[0])
            },
            Event::SetCycle {
                cycle: Cycle::Day,
                day_num: 3,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- DAY 3 --
    for client_id in &game.get_players(is_alive) {
        game.cast_vote(client_id, Some(client1_id)).unwrap();
    }

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client1_id,
                cycle: Cycle::Day,
                death_message: Box::from(DAY_DEATH_MESSAGES[0])
            },
            Event::GameWon {
                player_to_role: HashMap::from_iter([
                    (client7_id, SpecialRole::Mafia),
                    (client1_id, SpecialRole::Mafia),
                    (client2_id, SpecialRole::Doctor),
                    (client3_id, SpecialRole::Detective)
                ]),
                side: Allegiance::Villagers
            }
        ]
    );

    // -- VILLAGERS WIN --
    assert_eq!(game.get_winner(), Some(Allegiance::Villagers));
}

#[test_log::test]
fn test_game_e2e_doctor_investigator() {
    let mut client_state = ClientState::new();

    let (client1_id, _) = client_state.connect_client("garnet").unwrap();
    let (client2_id, _) = client_state.connect_client("amethyst").unwrap();
    let (client3_id, _) = client_state.connect_client("pearl").unwrap();
    let (client4_id, _) = client_state.connect_client("steven").unwrap();
    let (_client5_id, _) = client_state.connect_client("connie").unwrap();
    let (_client6_id, _) = client_state.connect_client("pink").unwrap();
    let (client7_id, _) = client_state.connect_client("blue").unwrap();

    let mut game = Game::start(
        GameConfig {
            start_cycle: Cycle::Day,
            time_for_day: Duration::from_secs(10),
            end_day_after_all_votes: true,
            time_for_night: Duration::from_secs(10),
            end_night_after_all_votes: true,
            num_special_roles: HashMap::from_iter([
                (SpecialRole::Mafia, 2),
                (SpecialRole::Detective, 1),
                (SpecialRole::Doctor, 1),
            ]),
            vote_grace_period: Duration::from_secs(0),
        },
        &client_state,
        StepRng::new(1, 1),
    )
    .unwrap();

    // Joined after the game started.
    let (_client8_id, _) = client_state.connect_client("yellow").unwrap();

    assert_eq!(
        *game.get_player_roles(),
        HashMap::from_iter([
            (client7_id, SpecialRole::Mafia),
            (client1_id, SpecialRole::Mafia),
            (client2_id, SpecialRole::Doctor),
            (client3_id, SpecialRole::Detective)
        ]),
    );

    // -- DAY 1 --
    for client_id in &game.get_players(is_alive) {
        game.cast_vote(client_id, None).unwrap();
    }

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::FailedVote {
                cycle: Cycle::Day,
                channel: EventChannel::Public,
            },
            Event::SetCycle {
                cycle: Cycle::Night,
                day_num: 1,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- NIGHT 1 --
    game.cast_vote(client7_id, Some(client4_id)).unwrap();
    game.cast_vote(client1_id, Some(client4_id)).unwrap();
    game.cast_vote(client2_id, Some(client4_id)).unwrap();
    game.cast_vote(client3_id, Some(client7_id)).unwrap();

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerInvestigated {
                actor: client3_id,
                target: client7_id,
                allegiance: Allegiance::Mafia,
            },
            Event::SetCycle {
                cycle: Cycle::Day,
                day_num: 2,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- DAY 2 --
    assert_eq!(game.get_players(is_alive).count(), 7);

    for client_id in &game.get_players(is_alive) {
        game.cast_vote(client_id, Some(client7_id)).unwrap();
    }

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client7_id,
                cycle: Cycle::Day,
                death_message: Box::from(DAY_DEATH_MESSAGES[0])
            },
            Event::SetCycle {
                cycle: Cycle::Night,
                day_num: 2,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- NIGHT 2 --
    assert_eq!(game.get_players(is_alive).count(), 6);

    game.cast_vote(client1_id, Some(client3_id)).unwrap();
    game.cast_vote(client2_id, Some(client3_id)).unwrap();
    game.cast_vote(client3_id, Some(client1_id)).unwrap();

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerInvestigated {
                actor: client3_id,
                target: client1_id,
                allegiance: Allegiance::Mafia,
            },
            Event::SetCycle {
                cycle: Cycle::Day,
                day_num: 3,
                start_time_unix_ts_secs: 0,
                duration_secs: 10
            }
        ]
    );

    // -- DAY 3 --
    assert_eq!(game.get_players(is_alive).count(), 6);

    for client_id in &game.get_players(is_alive) {
        game.cast_vote(client_id, Some(client1_id)).unwrap();
    }

    assert_eq!(
        game.poll_end_cycle(),
        vec![
            Event::PlayerKilled {
                player: client1_id,
                cycle: Cycle::Day,
                death_message: Box::from(DAY_DEATH_MESSAGES[0])
            },
            Event::GameWon {
                player_to_role: HashMap::from_iter([
                    (client7_id, SpecialRole::Mafia),
                    (client1_id, SpecialRole::Mafia),
                    (client2_id, SpecialRole::Doctor),
                    (client3_id, SpecialRole::Detective)
                ]),
                side: Allegiance::Villagers
            }
        ]
    );

    // -- VILLAGERS WIN --
    assert_eq!(game.get_players(is_alive).count(), 5);
    assert_eq!(game.get_winner(), Some(Allegiance::Villagers));
}
