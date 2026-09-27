use crate::error::AppError;
use crate::game::command::GameCommand;
use crate::game::event::{GameEvent, GameEventData, NewGameEvent};
use crate::game::game_state::GameState;
use crate::game::rules;
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, SignedDuration};
use time::ext::NumericalStdDuration;
use uuid::Uuid;
use crate::game::rules::TICK_SECONDS;

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
impl std::fmt::Display for GameStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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
    pub(crate) started_at: Option<OffsetDateTime>,
    pub(crate) events: Vec<GameEvent>,
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
            started_at: None,
            events: vec![],
        })
    }
    pub(crate) fn join(&mut self, player: Player) -> Result<(), AppError> {
        if self.status != GameStatus::Open {
            return Err(AppError::InvalidStatus {
                message: format!("Can't join a {} game", self.status),
            });
        }
        if self.players.iter().any(|p| p.id == player.id) {
            return Err(AppError::RuleViolation {
                message: "Player has already joined this game".to_owned(),
            });
        }
        if self.players.len() >= usize::from(self.max_players) {
            return Err(AppError::RuleViolation {
                message: format!("Game is full ({} players)", self.max_players),
            });
        }

        self.players.push(player);
        Ok(())
    }
    pub(crate) fn start(&mut self) -> Result<(), AppError> {
        if self.status != GameStatus::Open {
            return Err(AppError::InvalidStatus {
                message: format!("Can't start a {} game", self.status),
            });
        }
        if self.players.len() < 2 {
            return Err(AppError::InvalidStatus {
                message: format!(
                    "Need 2 players to start game, only found {}",
                    self.players.len()
                ),
            });
        }

        let player_ids: Vec<PlayerId> = self.players.iter().map(|p| p.id).collect();
        self.state = Some(rules::initial_state(&player_ids)?);
        self.status = GameStatus::Running;
        self.started_at = Some(OffsetDateTime::now_utc());
        Ok(())
    }
    pub(crate) fn restore(
        id: GameId,
        status: GameStatus,
        host: PlayerId,
        players: Vec<Player>,
        state: Option<GameState>,
        max_players: u8,
        created_at: OffsetDateTime,
        started_at: Option<OffsetDateTime>,
        events: Vec<GameEvent>,
    ) -> Result<Self, AppError> {
        if status == GameStatus::Running && !state.as_ref().is_some_and(|s| s.is_initialized()) {
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
            state,
            max_players,
            created_at,
            started_at,
            events,
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

    fn ensure_command_available(&self, player_id: PlayerId) -> Result<(), AppError> {
        if self.status != GameStatus::Running {
            return Err(AppError::InvalidCommand {
                message: format!("Game status {} invalid for commands", self.status),
            });
        }
        if (!self.players.iter().any(|p| p.id == player_id)) {
            return Err(AppError::InvalidCommand {
                message: format!("Player {} not found in game", player_id.to_uuid()),
            });
        }
        Ok(())
    }

    /// apply events that are due
    pub(crate) fn advanceTo(&mut self, now: OffsetDateTime)-> Result<Vec<GameEvent>, AppError> {
        let pending_events = self
            .events
            .iter_mut()
            .filter(|e| e.processed_at.is_none())
            .filter(|e| e.due_at <= now)
            .collect();

        pending_events.iter().for_each(|event| {
                self.process_event(event);
                event.processed_at = Some(now);
            });

        self.state_at(now);

        pending_events
    }

    pub(crate) fn process_event(
        &mut self,
        &mut event: &mut GameEvent,
    ) -> Result<GameEvent, AppError> {
        self.state_at(event.due_at);
        rules::process_event(self.state.as_mut(), event)
            event.processed_at
        Ok(event)
    }

    pub(crate) fn apply_command(
        &mut self,
        player_id: PlayerId,
        command: GameCommand,
        now: OffsetDateTime,
    ) -> Result<Vec<NewGameEvent>, AppError> {
        self.ensure_command_available(player_id)?;
        self.advanceTo(now)

        let new_events = rules::execute_command(self.state.as_mut(), command)
            ?.into_iter().map(|e|{NewGameEvent {
                game_id: self.id,
                command_id: None,
                data: e.data,
                due_tick: e.due_tick,
                due_at: now.saturating_add(self.state.unwrap().tick.saturating_mul(TICK_SECONDS).std_seconds())
            }}).collect();

    }
}
