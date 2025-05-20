use std::collections::HashMap;
use std::time::Duration;

use insta::assert_json_snapshot;
use rand::rngs::mock::StepRng;

use crate::MafiaGameServer;
use crate::MafiaGameServerConfig;
use crate::game::GameConfig;
use mafia_game_lib::Allegiance;
use mafia_game_lib::Cycle;
use mafia_game_lib::SpecialRole;

#[test_log::test]
fn test_server_messages() {
    let server = MafiaGameServer::new(MafiaGameServerConfig {
        max_client_inactive_time: Duration::from_secs(300),
        randomize_death_message: false,
    });

    let (client0_id, client0_token) = server.connect_client("garnet").unwrap();
    let (client1_id, client1_token) = server.connect_client("amethyst").unwrap();
    let (client2_id, client2_token) = server.connect_client("pearl").unwrap();
    let (client3_id, client3_token) = server.connect_client("steven").unwrap();
    let (_client4_id, client4_token) = server.connect_client("connie").unwrap();
    let (_client5_id, client5_token) = server.connect_client("pink").unwrap();
    let (client6_id, client6_token) = server.connect_client("blue").unwrap();

    server.broadcast_message(Box::from("game is starting!"));

    server
        .start_game(
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
            StepRng::new(1, 1),
        )
        .unwrap();

    // Joined after the game started.
    let (_client7_id, client7_token) = server.connect_client("yellow").unwrap();

    assert_eq!(
        *server
            .0
            .read()
            .unwrap()
            .active_game
            .as_ref()
            .unwrap()
            .get_player_roles(),
        HashMap::from_iter([
            (client6_id, SpecialRole::Mafia),
            (client0_id, SpecialRole::Mafia),
            (client1_id, SpecialRole::Doctor),
            (client2_id, SpecialRole::Detective)
        ]),
    );

    // -- DAY 1 --
    server
        .send_message(client0_token, "hey everyone!".into())
        .unwrap();
    server
        .send_message(client1_token, "hey hey".into())
        .unwrap();
    server.send_message(client3_token, "hi!".into()).unwrap();
    server
        .send_message(client7_token, "oh I joined late :(".into())
        .unwrap();

    server.cast_vote(client0_token, None).unwrap();
    server.cast_vote(client1_token, None).unwrap();
    server.cast_vote(client2_token, None).unwrap();
    server.cast_vote(client3_token, None).unwrap();
    server.cast_vote(client4_token, None).unwrap();
    server.cast_vote(client5_token, None).unwrap();
    server.cast_vote(client6_token, None).unwrap();
    server.cast_vote(client7_token, None).unwrap_err();

    // -- NIGHT 1 --
    server
        .send_message(client6_token, "let's kill steven".into())
        .unwrap();
    server.send_message(client0_token, "okay".into()).unwrap();
    server
        .send_message(client1_token, "wonder what's gonna happen tonight".into())
        .unwrap();
    server
        .send_message(client7_token, "looks like fun!".into())
        .unwrap();

    server.cast_vote(client6_token, Some(client3_id)).unwrap();
    server.cast_vote(client0_token, Some(client3_id)).unwrap();

    // Joined after the game started.
    let (_client8_id, client8_token) = server.connect_client("white").unwrap();

    server.cast_vote(client1_token, None).unwrap();
    server.cast_vote(client2_token, Some(client6_id)).unwrap();

    // -- DAY 2 --
    server.send_message(client3_token, "wtf".into()).unwrap();
    server
        .send_message(client7_token, "welcome to the club".into())
        .unwrap();
    server
        .send_message(client2_token, "detective here, blue is mafia".into())
        .unwrap();
    server
        .send_message(client0_token, "how can we trust you?".into())
        .unwrap();
    server.send_message(client2_token, "idk".into()).unwrap();
    server
        .send_message(client4_token, "seems legit".into())
        .unwrap();
    server
        .send_message(client6_token, "hold on!!".into())
        .unwrap();

    server.cast_vote(client0_token, Some(client2_id)).unwrap();
    server.cast_vote(client1_token, Some(client6_id)).unwrap();
    server.cast_vote(client2_token, Some(client6_id)).unwrap();
    server.cast_vote(client3_token, None).unwrap_err();
    server.cast_vote(client4_token, Some(client6_id)).unwrap();
    server.cast_vote(client5_token, Some(client6_id)).unwrap();
    server.cast_vote(client6_token, Some(client2_id)).unwrap();
    server.cast_vote(client7_token, None).unwrap_err();

    // -- NIGHT 2 --
    server.send_message(client6_token, "damn".into()).unwrap();
    server
        .send_message(client0_token, "sorry blue, i tried".into())
        .unwrap();
    server
        .send_message(client3_token, "you killed me?".into())
        .unwrap();
    server
        .send_message(client2_token, "garnet seems sus".into())
        .unwrap();
    server
        .send_message(client6_token, "nothing personal".into())
        .unwrap();
    server
        .send_message(client1_token, "imma protect pearl".into())
        .unwrap();

    server.cast_vote(client0_token, Some(client2_id)).unwrap();
    server.cast_vote(client1_token, Some(client2_id)).unwrap();
    server.cast_vote(client2_token, Some(client0_id)).unwrap();

    // -- DAY 3 --
    server
        .send_message(client7_token, "tragic lol".into())
        .unwrap();
    server
        .send_message(client5_token, "wow good job doctor!".into())
        .unwrap();
    server
        .send_message(client2_token, "garnet is the other mafia".into())
        .unwrap();
    server
        .send_message(client6_token, "oh we lost rip".into())
        .unwrap();
    server.send_message(client0_token, "no!".into()).unwrap();

    server.cast_vote(client0_token, Some(client2_id)).unwrap();
    server.cast_vote(client1_token, Some(client0_id)).unwrap();
    server.cast_vote(client2_token, Some(client0_id)).unwrap();
    server.cast_vote(client3_token, None).unwrap_err();
    server.cast_vote(client4_token, Some(client0_id)).unwrap();
    server.cast_vote(client5_token, Some(client0_id)).unwrap();
    server.cast_vote(client6_token, None).unwrap_err();
    server.cast_vote(client7_token, None).unwrap_err();

    // -- VILLAGERS WIN --
    server.broadcast_message(Box::from("villagers won"));

    server.send_message(client0_token, "shit".into()).unwrap();
    server.send_message(client6_token, "gg".into()).unwrap();
    server
        .send_message(client2_token, "yeah gg all".into())
        .unwrap();
    server
        .send_message(client7_token, "hope to join the next one!".into())
        .unwrap();

    assert_eq!(
        server
            .0
            .read()
            .unwrap()
            .active_game
            .as_ref()
            .unwrap()
            .get_winner(),
        Some(Allegiance::Villagers)
    );

    insta::with_settings!({sort_maps => true}, {
        assert_json_snapshot!(server.take_events(client0_token).unwrap());
        assert_json_snapshot!(server.take_events(client1_token).unwrap());
        assert_json_snapshot!(server.take_events(client2_token).unwrap());
        assert_json_snapshot!(server.take_events(client3_token).unwrap());
        assert_json_snapshot!(server.take_events(client4_token).unwrap());
        assert_json_snapshot!(server.take_events(client5_token).unwrap());
        assert_json_snapshot!(server.take_events(client6_token).unwrap());
        assert_json_snapshot!(server.take_events(client7_token).unwrap());
        assert_json_snapshot!(server.take_events(client8_token).unwrap());
    });
}
