//! Constants for the Mafia game.

/// Icon identifier used for clients.
pub const PLAYER_EMOJIS: &[char] = &[
    '😡', '👺', '💩', '🐷', '🙈', '🌸', '🌚', '👍', '💜', '🎩', '🌈', '🧮', '💡', '🐸', '🍣', '🍩',
    '🥑', '🎲', '🎳', '🍄',
];

/// Night death message used in the form of:
/// <PLAYER> <DEATH_MESSAGE> the next morning.
#[allow(dead_code)]
pub const NIGHT_DEATH_MESSAGES: &[&str] = &[
    "was found strangled by an untyped python",
    "was found brutally beat with a mechanical keyboard",
    "was found poisoned from eating expired ketchup",
    "never made it home because of 101 traffic",
    "was found pummelled by what appears to have been a gorilla",
    "was found unresponsive next to a beer tower",
];

/// Day death message used in the form of:
/// <PLAYER> <DEATH_MESSAGE> that day.
#[allow(dead_code)]
pub const DAY_DEATH_MESSAGES: &[&str] = &["was hung for their unforgivable sins"];
