use crate::error::AppError;
use crate::game::game_state::GameState;
use crate::game::rules;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(transparent)]
pub struct PlayerId(Uuid);

impl PlayerId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
    pub(crate) const fn to_uuid(self) -> Uuid {
        self.0
    }
    pub(crate) const fn from_uuid(id: Uuid) -> PlayerId {
        PlayerId(id)
    }
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameStatus {
    Open,
    Running,
    Closed,
}

impl GameStatus {
    pub(crate) const fn as_str(&self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Running => "Running",
            Self::Closed => "Closed",
        }
    }
}
impl std::str::FromStr for GameStatus {
    type Err = AppError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Open" => Ok(Self::Open),
            "Running" => Ok(Self::Running),
            "Closed" => Ok(Self::Closed),
            _ => Err(AppError::invalid_data(format!(
                "invalid game status: {}",
                value
            ))),
        }
    }
}

#[derive(Clone)]
pub struct Player {
    pub(crate) id: PlayerId,
    pub(crate) name: String,
}
impl Player {
    pub fn new(name: String) -> Self {
        Player {
            id: PlayerId(Uuid::new_v4()),
            name,
        }
    }
}

pub struct Game {
    pub(crate) id: GameId,
    pub(crate) status: GameStatus,
    pub(crate) host: PlayerId,
    pub(crate) players: Vec<Player>,
    state: Option<GameState>,
    pub(crate) max_players: u8,
    pub(crate) created_at: OffsetDateTime,
}
impl Game {
    pub(crate) fn initiate(max_players: u8, host: Player) -> Result<Self, AppError> {
        Ok(Self {
            id: GameId::new(),
            status: GameStatus::Open,
            host: host.id,
            players: vec![host],
            state: None,
            max_players,
            created_at: OffsetDateTime::now_utc(),
        })
    }
    pub(crate) fn start(&mut self) -> Result<(), AppError> {
        let playerIds: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
        self.state = Some(rules::initial_state(&playerIds)?);
        self.status = GameStatus::Running;
        Ok(())
    }
    pub(crate) fn restore(
        id: GameId,
        status: GameStatus,
        host: PlayerId,
        players: Vec<Player>,
        state: GameState,
        max_players: u8,
        created_at: OffsetDateTime,
    ) -> Result<Self, AppError> {
        if status == GameStatus::Running && !state.is_initialized() {
            return Err(AppError::InvalidState {
                message: "Game is running, but state is not initialized".to_owned(),
            });
        }
        if !players.iter().any(|player| player.id == host) {
            return Err(AppError::invalid_data("host is not a player in the game"));
        }
        if players.len() > usize::from(max_players) {
            return Err(AppError::invalid_data(format!(
                "game has {} players but allows at most {}",
                players.len(),
                max_players
            )));
        }

        Ok(Self {
            id,
            status,
            host,
            players,
            state: Some(state),
            max_players,
            created_at,
        })
    }
    /// The only way to read state.
    pub(crate) fn state_at(&mut self, now: OffsetDateTime) -> Result<&GameState, AppError> {
        let tick = rules::tick_at(self.created_at, now);
        let state = self.state.as_mut().ok_or(AppError::InvalidState {
            message: "game has not started".to_owned(),
        })?;
        rules::advance_to(state, tick);
        Ok(state)
    }

    /// Persistence only — writes the state at whatever tick it holds.
    pub(crate) fn persisted_state(&self) -> Option<&GameState> {
        self.state.as_ref()
    }
}
