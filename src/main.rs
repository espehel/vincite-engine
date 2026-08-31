mod api;
mod db;
mod error;
mod game;

use crate::api::create_games_router;

use crate::db::establish_connection;
use axum::Router;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")?;
    let pool = establish_connection(&database_url).await?;

    let app = Router::new()
        .nest("/games", create_games_router())
        .with_state(pool);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    axum::serve(listener, app).await.unwrap();

    Ok(())
}
