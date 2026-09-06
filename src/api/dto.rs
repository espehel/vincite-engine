use crate::error::AppError;
use crate::game::game::{Game, GameId, GameStatus, Player, PlayerId};
use axum::Json;
use axum::http::StatusCode;

pub type ApiPostResult<T> = Result<(StatusCode, Json<T>), AppError>;
pub type ApiResult<T> = Result<Json<T>, AppError>;

#[derive(serde::Deserialize)]
pub struct CreateGameRequest {
    pub maximum_players: u8,
    pub host: PlayerId,
}
#[derive(serde::Serialize)]
pub struct GameDto {
    pub id: GameId,
    pub status: GameStatus,
}
impl From<Game> for GameDto {
    fn from(game: Game) -> Self {
        Self {
            id: game.id,
            status: game.status,
        }
    }
}

#[derive(serde::Deserialize)]
pub struct CreatePlayerRequest {
    pub name: String,
}

#[derive(serde::Serialize)]
pub struct PlayerDto {
    pub id: PlayerId,
    pub name: String,
}
impl From<Player> for PlayerDto {
    fn from(player: Player) -> Self {
        Self {
            id: player.id,
            name: player.name,
        }
    }
}
