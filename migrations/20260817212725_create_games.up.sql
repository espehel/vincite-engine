CREATE TABLE games (
    id UUID PRIMARY KEY,
    status TEXT NOT NULL,
    state JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE game_players (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE game_events (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL
);
