use crate::error::AppError;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GameId(Uuid);

impl GameId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    pub(crate) const fn to_uuid(self) -> Uuid {
        self.0
    }
    pub(crate) const fn from_uuid(id: Uuid) -> GameId {
        GameId(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlayerId(Uuid);
impl PlayerId {
    pub(crate) const fn into_uuid(self) -> Uuid {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameStatus {
    Open,
    Running,
    Closed,
}

impl GameStatus {
    pub(crate) const fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Running => "running",
            Self::Closed => "closed",
        }
    }
}
impl std::str::FromStr for GameStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "open" => Ok(Self::Open),
            "running" => Ok(Self::Running),
            "closed" => Ok(Self::Closed),
            _ => Err(AppError::invalid_data(format!(
                "invalid game status: {}",
                value
            ))),
        }
    }
}

pub struct Player {
    id: PlayerId,
    name: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct GameState {}
impl GameState {
    pub(crate) const fn is_initialized(&self) -> bool {
        true
    }
}

pub struct Game {
    pub(crate) id: GameId,
    pub(crate) status: GameStatus,
    pub(crate) players: Vec<Player>,
    pub(crate) state: GameState,
    pub(crate) max_players: u8,
    pub(crate) created_at: OffsetDateTime,
}
impl Game {
    pub(crate) fn initiate(max_players: u8) -> Result<Self, AppError> {
        Ok(Self {
            id: GameId::new(),
            status: GameStatus::Open,
            players: vec![],
            state: GameState {},
            max_players,
            created_at: OffsetDateTime::now_utc(),
        })
    }
    pub(crate) fn restore(
        id: GameId,
        status: GameStatus,
        state: GameState,
        max_players: u8,
        created_at: OffsetDateTime,
    ) -> Result<Self, AppError> {
        if status == GameStatus::Running && !state.is_initialized() {
            return Err(AppError::InvalidState {
                message: "Game is running, but state is not initialized".to_owned(),
            });
        }

        Ok(Self {
            id,
            status,
            state,
            players: Vec::with_capacity(usize::from(max_players)),
            max_players,
            created_at,
        })
    }
}
