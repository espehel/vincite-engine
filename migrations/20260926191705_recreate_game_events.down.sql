DROP TABLE game_events;

CREATE TABLE game_events (
    id UUID PRIMARY KEY,
    kind TEXT NOT NULL
);
