use crate::game::game::GameId;
use crate::game::game_state::{Building, SettlementId};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub(crate) enum GameEventData {
    BuildingUpgradeCompleted {
        settlement_id: SettlementId,
        building: Building,
        to_level: u8,
    },
}

#[derive(Clone)]
pub(crate) struct GameEvent {
    pub id: i64,
    pub game_id: GameId,
    pub command_id: Option<i64>,
    pub data: GameEventData,
    pub due_tick: i64,
    pub due_at: OffsetDateTime,
    pub processed_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

pub(crate) struct ScheduledEvent {
    pub data: GameEventData,
    pub due_tick: i64,
}

pub(crate) struct NewGameEvent {
    pub game_id: GameId,
    pub command_id: Option<i64>,
    pub data: GameEventData,
    pub due_tick: i64,
    pub due_at: OffsetDateTime,
}
