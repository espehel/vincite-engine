CREATE TABLE players (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL
);

-- The original game_players was a standalone list of names, not a link between
-- a game and its players. Replace it.
DROP TABLE game_players;

CREATE TABLE game_players (
    game_id UUID NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    player_id UUID NOT NULL REFERENCES players (id) ON DELETE CASCADE,
    is_host BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (game_id, player_id)
);

CREATE INDEX game_players_player_id_idx ON game_players (player_id);

-- The host is one of the game's players, so hosting is a property of the
-- membership rather than a column on games. That keeps "the host plays in the
-- game it hosts" true by construction, and leaves at most one host per game as
-- the only rule left to enforce.
CREATE UNIQUE INDEX game_players_one_host_per_game
    ON game_players (game_id) WHERE is_host;
