use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rand::rngs::mock::StepRng;

use crate::MafiaGameServer;
use crate::MafiaGameServerConfig;
use crate::client::ClientId;
use crate::client::Entity;
use crate::client::Message;
use crate::client::MessageChannel;
use crate::game::Allegiance;
use crate::game::Cycle;
use crate::game::GameConfig;
use crate::game::SpecialRole;

#[test_log::test]
fn test_server_messages() {
    let server = MafiaGameServer::new(MafiaGameServerConfig {
        max_client_inactive_time: Duration::from_secs(300),
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
            .get_cycle(),
        Cycle::Won(Allegiance::Villagers)
    );

    use Entity::*;
    use MessageChannel::*;

    let all_start = [
        Message {
            channel: Public,
            contents: Box::from("game is starting!"),
            from: System,
        },
        Message {
            channel: Public,
            contents: Box::from("hey everyone!"),
            from: Client(ClientId(0)),
        },
        Message {
            channel: Public,
            contents: Box::from("hey hey"),
            from: Client(ClientId(1)),
        },
        Message {
            channel: Public,
            contents: Box::from("hi!"),
            from: Client(ClientId(3)),
        },
    ];

    let all_end = [
        Message {
            channel: Public,
            contents: Box::from("villagers won"),
            from: System,
        },
        Message {
            channel: Public,
            contents: Box::from("shit"),
            from: Client(ClientId(0)),
        },
        Message {
            channel: Public,
            contents: Box::from("gg"),
            from: Client(ClientId(6)),
        },
        Message {
            channel: Public,
            contents: Box::from("yeah gg all"),
            from: Client(ClientId(2)),
        },
        Message {
            channel: Public,
            contents: Box::from("hope to join the next one!"),
            from: Client(ClientId(7)),
        },
    ];

    assert_eq!(
        server.take_messages(client0_token).unwrap(),
        Box::from_iter(
            all_start
                .clone()
                .into_iter()
                .chain([
                    Message {
                        channel: Mafia,
                        contents: Box::from("let's kill steven"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Mafia,
                        contents: Box::from("okay"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("detective here, blue is mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("how can we trust you?"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("idk"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("seems legit"),
                        from: Client(ClientId(4))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("hold on!!"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Mafia,
                        contents: Box::from("sorry blue, i tried"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("wow good job doctor!"),
                        from: Client(ClientId(5))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("garnet is the other mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("no!"),
                        from: Client(ClientId(0))
                    },
                ])
                .chain(all_end.clone())
                .into_iter()
                .map(|v| Arc::new(v))
        ),
    );

    assert_eq!(
        server.take_messages(client1_token).unwrap(),
        Box::from_iter(
            all_start
                .clone()
                .into_iter()
                .chain([
                    Message {
                        channel: Spectator,
                        contents: Box::from("wonder what's gonna happen tonight"),
                        from: Client(ClientId(1))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("detective here, blue is mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("how can we trust you?"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("idk"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("seems legit"),
                        from: Client(ClientId(4))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("hold on!!"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("imma protect pearl"),
                        from: Client(ClientId(1))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("wow good job doctor!"),
                        from: Client(ClientId(5))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("garnet is the other mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("no!"),
                        from: Client(ClientId(0))
                    },
                ])
                .chain(all_end.clone())
                .into_iter()
                .map(|v| Arc::new(v))
        ),
    );

    assert_eq!(
        server.take_messages(client2_token).unwrap(),
        Box::from_iter(
            all_start
                .clone()
                .into_iter()
                .chain([
                    Message {
                        channel: Public,
                        contents: Box::from("detective here, blue is mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("how can we trust you?"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("idk"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("seems legit"),
                        from: Client(ClientId(4))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("hold on!!"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("garnet seems sus"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("wow good job doctor!"),
                        from: Client(ClientId(5))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("garnet is the other mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("no!"),
                        from: Client(ClientId(0))
                    },
                ])
                .chain(all_end.clone())
                .into_iter()
                .map(|v| Arc::new(v))
        )
    );

    assert_eq!(
        server.take_messages(client3_token).unwrap(),
        Box::from_iter(
            all_start
                .clone()
                .into_iter()
                .chain([
                    Message {
                        channel: Spectator,
                        contents: Box::from("wtf"),
                        from: Client(ClientId(3))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("welcome to the club"),
                        from: Client(ClientId(7))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("detective here, blue is mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("how can we trust you?"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("idk"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("seems legit"),
                        from: Client(ClientId(4))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("hold on!!"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("damn"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Mafia,
                        contents: Box::from("sorry blue, i tried"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("you killed me?"),
                        from: Client(ClientId(3))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("garnet seems sus"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("nothing personal"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("imma protect pearl"),
                        from: Client(ClientId(1))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("tragic lol"),
                        from: Client(ClientId(7))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("wow good job doctor!"),
                        from: Client(ClientId(5))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("garnet is the other mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("oh we lost rip"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("no!"),
                        from: Client(ClientId(0))
                    }
                ])
                .chain(all_end.clone())
                .into_iter()
                .map(|v| Arc::new(v))
        )
    );

    assert_eq!(
        server.take_messages(client4_token).unwrap(),
        Box::from_iter(
            all_start
                .clone()
                .into_iter()
                .chain([
                    Message {
                        channel: Public,
                        contents: Box::from("detective here, blue is mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("how can we trust you?"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("idk"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("seems legit"),
                        from: Client(ClientId(4))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("hold on!!"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("wow good job doctor!"),
                        from: Client(ClientId(5))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("garnet is the other mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("no!"),
                        from: Client(ClientId(0))
                    }
                ])
                .chain(all_end.clone())
                .into_iter()
                .map(|v| Arc::new(v))
        ),
    );

    assert_eq!(
        server.take_messages(client5_token).unwrap(),
        Box::from_iter(
            all_start
                .clone()
                .into_iter()
                .chain([
                    Message {
                        channel: Public,
                        contents: Box::from("detective here, blue is mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("how can we trust you?"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("idk"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("seems legit"),
                        from: Client(ClientId(4))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("hold on!!"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("wow good job doctor!"),
                        from: Client(ClientId(5))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("garnet is the other mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("no!"),
                        from: Client(ClientId(0))
                    }
                ])
                .chain(all_end.clone())
                .into_iter()
                .map(|v| Arc::new(v))
        ),
    );

    assert_eq!(
        server.take_messages(client6_token).unwrap(),
        Box::from_iter(
            all_start
                .clone()
                .into_iter()
                .chain([
                    Message {
                        channel: Mafia,
                        contents: Box::from("let's kill steven"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Mafia,
                        contents: Box::from("okay"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("detective here, blue is mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("how can we trust you?"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("idk"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("seems legit"),
                        from: Client(ClientId(4))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("hold on!!"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("damn"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Mafia,
                        contents: Box::from("sorry blue, i tried"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("you killed me?"),
                        from: Client(ClientId(3))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("garnet seems sus"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("nothing personal"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("imma protect pearl"),
                        from: Client(ClientId(1))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("tragic lol"),
                        from: Client(ClientId(7))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("wow good job doctor!"),
                        from: Client(ClientId(5))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("garnet is the other mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("oh we lost rip"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("no!"),
                        from: Client(ClientId(0))
                    }
                ])
                .chain(all_end.clone())
                .into_iter()
                .map(|v| Arc::new(v))
        ),
    );

    assert_eq!(
        server.take_messages(client7_token).unwrap(),
        Box::from_iter(
            all_start
                .clone()
                .into_iter()
                .skip(1)
                .chain([
                    Message {
                        channel: Spectator,
                        contents: Box::from("oh I joined late :("),
                        from: Client(ClientId(7))
                    },
                    Message {
                        channel: Mafia,
                        contents: Box::from("let's kill steven"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Mafia,
                        contents: Box::from("okay"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("wonder what's gonna happen tonight"),
                        from: Client(ClientId(1))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("looks like fun!"),
                        from: Client(ClientId(7))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("wtf"),
                        from: Client(ClientId(3))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("welcome to the club"),
                        from: Client(ClientId(7))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("detective here, blue is mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("how can we trust you?"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("idk"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("seems legit"),
                        from: Client(ClientId(4))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("hold on!!"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("damn"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Mafia,
                        contents: Box::from("sorry blue, i tried"),
                        from: Client(ClientId(0))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("you killed me?"),
                        from: Client(ClientId(3))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("garnet seems sus"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("nothing personal"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("imma protect pearl"),
                        from: Client(ClientId(1))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("tragic lol"),
                        from: Client(ClientId(7))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("wow good job doctor!"),
                        from: Client(ClientId(5))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("garnet is the other mafia"),
                        from: Client(ClientId(2))
                    },
                    Message {
                        channel: Spectator,
                        contents: Box::from("oh we lost rip"),
                        from: Client(ClientId(6))
                    },
                    Message {
                        channel: Public,
                        contents: Box::from("no!"),
                        from: Client(ClientId(0))
                    }
                ])
                .chain(all_end.clone())
                .into_iter()
                .map(|v| Arc::new(v))
        ),
    );
}
