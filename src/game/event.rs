use crate::game::game::GameId;
use time::OffsetDateTime;

pub(crate) struct GameEvent {
    pub id: usize,
    pub game_id: GameId,
    pub command_id: usize,
    pub kind: String,
    pub payload: String,
    due_tick: usize,
    due_at: OffsetDateTime,
    processed_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
}
