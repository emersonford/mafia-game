use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::client::ClientState;
use crate::error::MafiaGameError;
use mafia_game_lib::Entity;
use mafia_game_lib::Event;
use mafia_game_lib::EventChannel;
use mafia_game_lib::Message;

#[test]
pub fn test_client_registration() {
    let mut client_state = ClientState::new();

    let (client1_id, client1_session_token) = client_state.connect_client("hello").unwrap();
    let (client2_id, client2_session_token) = client_state.connect_client("world").unwrap();

    assert_eq!(
        client_state.auth_client(client1_session_token).unwrap(),
        client1_id
    );
    assert_eq!(
        client_state.auth_client(client2_session_token).unwrap(),
        client2_id
    );

    assert_eq!(
        client_state
            .get_client(client1_id)
            .unwrap()
            .get_info()
            .name
            .as_ref(),
        "hello"
    );
    assert_eq!(
        client_state
            .get_client(client2_id)
            .unwrap()
            .get_info()
            .name
            .as_ref(),
        "world"
    );

    {
        let clients = client_state.list_clients();

        assert_eq!(
            *clients,
            HashMap::from_iter([
                (Arc::from("hello"), client1_id),
                (Arc::from("world"), client2_id)
            ])
        );
    }

    assert!(matches!(
        client_state.connect_client("hello"),
        Err(MafiaGameError::ClientNameRegistered(_))
    ));

    {
        let clients = client_state.list_clients();

        assert_eq!(
            *clients,
            HashMap::from_iter([
                (Arc::from("hello"), client1_id),
                (Arc::from("world"), client2_id)
            ])
        );
    }

    assert!(client_state.disconnect_client(client1_id).is_ok());
    assert!(matches!(
        client_state.auth_client(client1_session_token),
        Err(MafiaGameError::ClientDisconnected(_))
    ));

    {
        let clients = client_state.list_clients();

        assert_eq!(
            *clients,
            HashMap::from_iter([
                (Arc::from("hello"), client1_id),
                (Arc::from("world"), client2_id)
            ])
        );
    }

    let (tmp_id, tmp_session_token) = client_state.connect_client("hello").unwrap();
    assert_eq!(client1_id, tmp_id);
    assert_ne!(client1_session_token, tmp_session_token);

    {
        let clients = client_state.list_clients();

        assert_eq!(
            *clients,
            HashMap::from_iter([
                (Arc::from("hello"), client1_id),
                (Arc::from("world"), client2_id)
            ])
        );
    }

    client_state.purge_disconnected_clients(Duration::from_secs(10));

    {
        let clients = client_state.list_clients();

        assert_eq!(
            *clients,
            HashMap::from_iter([
                (Arc::from("hello"), client1_id),
                (Arc::from("world"), client2_id)
            ])
        );
    }

    assert!(client_state.disconnect_client(client1_id).is_ok());
    client_state.purge_disconnected_clients(Duration::from_secs(10));

    {
        let clients = client_state.list_clients();

        assert_eq!(
            *clients,
            HashMap::from_iter([(Arc::from("world"), client2_id)])
        );
    }

    assert!(client_state.disconnect_client(client1_id).is_err());
    assert!(client_state.disconnect_client(client2_id).is_ok());
    client_state.purge_disconnected_clients(Duration::from_secs(10));

    {
        let clients = client_state.list_clients();

        assert_eq!(*clients, HashMap::new());
    }
}

#[test]
fn test_messages() {
    let mut client_state = ClientState::new();

    let (client1_id, _) = client_state.connect_client("hello").unwrap();
    let (client2_id, _) = client_state.connect_client("world").unwrap();

    client_state.send_event(
        [client1_id, client2_id].into_iter().collect(),
        Message {
            channel: EventChannel::Public,
            contents: Box::from("hello world"),
            from: Entity::Client(client1_id),
        },
    );

    client_state.send_event(
        [client2_id].into_iter().collect(),
        Message {
            channel: EventChannel::Mafia,
            contents: Box::from("just mafia"),
            from: Entity::Client(client2_id),
        },
    );

    assert_eq!(
        client_state.take_events(client1_id),
        [Message {
            channel: EventChannel::Public,
            contents: Box::from("hello world"),
            from: Entity::Client(client1_id)
        }]
        .into_iter()
        .map(|v| Arc::new(Event::MessageReceived(v)))
        .collect()
    );

    assert_eq!(
        client_state.take_events(client2_id),
        [
            Message {
                channel: EventChannel::Public,
                contents: Box::from("hello world"),
                from: Entity::Client(client1_id)
            },
            Message {
                channel: EventChannel::Mafia,
                contents: Box::from("just mafia"),
                from: Entity::Client(client2_id),
            },
        ]
        .into_iter()
        .map(|v| Arc::new(Event::MessageReceived(v)))
        .collect()
    );

    assert_eq!(client_state.take_events(client1_id), Box::from([]));
    assert_eq!(client_state.take_events(client2_id), Box::from([]));

    client_state.send_event(
        [client1_id, client2_id].into_iter().collect(),
        Message {
            channel: EventChannel::Public,
            contents: Box::from("foobar"),
            from: Entity::Client(client1_id),
        },
    );

    client_state.send_event(
        [client1_id].into_iter().collect(),
        Message {
            channel: EventChannel::Spectator,
            contents: Box::from("just spectator"),
            from: Entity::Client(client1_id),
        },
    );

    assert_eq!(
        client_state.take_events(client1_id),
        [
            Message {
                channel: EventChannel::Public,
                contents: Box::from("foobar"),
                from: Entity::Client(client1_id)
            },
            Message {
                channel: EventChannel::Spectator,
                contents: Box::from("just spectator"),
                from: Entity::Client(client1_id),
            },
        ]
        .into_iter()
        .map(|v| Arc::new(Event::MessageReceived(v)))
        .collect()
    );

    assert_eq!(
        client_state.take_events(client2_id),
        [Message {
            channel: EventChannel::Public,
            contents: Box::from("foobar"),
            from: Entity::Client(client1_id)
        },]
        .into_iter()
        .map(|v| Arc::new(Event::MessageReceived(v)))
        .collect()
    );
}
