use thiserror::Error;

use crate::client::ClientId;
use crate::client::SessionToken;

#[derive(Error, Debug)]
pub enum MafiaGameError {
    #[error("client name '{0}' is already registered")]
    ClientNameRegistered(String),
    #[error("invalid session token provided '{0}'")]
    InvalidSessionToken(SessionToken),
    #[error("{0:?} is not registered")]
    InvalidClientId(ClientId),
    #[error("too many clients are registered")]
    TooManyClientsRegistered,
    #[error("not enough clients: {0}")]
    NotEnoughPlayers(String),
    #[error("invalid game config: {0}")]
    InvalidGameConfig(String),
    #[error("invalid vote: {0}")]
    InvalidVote(String),
    #[error("there is a game already in progress")]
    GameInProgress,
    #[error("no game is in progress")]
    NoGameInProgress,
}
