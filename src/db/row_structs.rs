use crate::game::event::GameEvent;
use crate::game::game::{GameId, Player, PlayerId};
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
pub(crate) struct GameRow {
    pub id: Uuid,
    pub status: String,
    pub state: serde_json::Value,
    pub maximum_players: i16,
    pub created_at: OffsetDateTime,
    pub started_at: Option<OffsetDateTime>,
}

#[derive(sqlx::FromRow)]
pub(crate) struct GamePlayerRow {
    pub game_id: Uuid,
    pub id: Uuid,
    pub name: String,
    pub is_host: bool,
}

#[derive(sqlx::FromRow)]
pub(crate) struct PlayerRow {
    pub id: uuid::Uuid,
    pub name: String,
}
impl From<PlayerRow> for Player {
    fn from(row: PlayerRow) -> Self {
        Self {
            id: PlayerId::from_uuid(row.id),
            name: row.name,
        }
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct GameEventRow {
    pub id: i64,
    pub game_id: Uuid,
    pub command_id: Option<i64>,
    pub kind: String,
    pub payload: serde_json::Value,
    pub due_tick: i64,
    pub due_at: OffsetDateTime,
    pub processed_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}
impl From<GameEventRow> for GameEvent {
    fn from(row: GameEventRow) -> Self {
        Self {
            id: row.id,
            game_id: GameId::from_uuid(row.game_id),
            command_id: row.command_id,
            processed_at: row.processed_at,
            due_tick: row.due_tick,
            due_at: row.due_at,
            created_at: row.created_at,
            data: serde_json::from_value(row.payload).expect("Failed to deserialize GameEventData"),
        }
    }
}

#[derive(sqlx::FromRow)]
pub(crate) struct GameCommandRow {
    pub id: i64,
    pub game_id: Uuid,
    pub player_id: Uuid,
    pub issued_at_tick: i64,
    pub kind: String,
    pub payload: serde_json::Value,
    pub created_at: OffsetDateTime,
}
