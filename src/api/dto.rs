use crate::game::game::GameId;

#[derive(serde::Deserialize)]
pub struct CreateGameRequest {
    pub maximum_players: u8,
}

#[derive(serde::Serialize)]
pub struct CreateGameResponse {
    pub id: GameId,
}
