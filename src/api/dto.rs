use crate::game::game::{GameId, GameStatus};

#[derive(serde::Deserialize)]
pub struct CreateGameRequest {
    pub maximum_players: u8,
}

#[derive(serde::Serialize)]
pub struct CreateGameResponse {
    pub id: GameId,
}

#[derive(serde::Serialize)]
pub struct GetGameResponse {
    pub id: GameId,
    pub status: GameStatus,
}

#[derive(serde::Serialize)]
pub struct GetGamesResponse {
    pub games: Vec<GetGameResponse>,
}
