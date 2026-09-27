-- Commands are applied synchronously in the request that issues them; this
-- table is the log of the ones that were accepted. A rejected command changes
-- nothing, so it is not recorded.
CREATE TABLE game_commands (
    -- Commands against one game serialize on the game row lock, so identity
    -- order is the order they were applied in.
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    game_id UUID NOT NULL,
    player_id UUID NOT NULL,
    issued_at_tick BIGINT NOT NULL CHECK (issued_at_tick >= 0),
    kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- References the membership rather than games and players separately, so
    -- only a player in the game can have issued a command against it.
    FOREIGN KEY (game_id, player_id)
        REFERENCES game_players (game_id, player_id) ON DELETE CASCADE
);

CREATE INDEX game_commands_game_id_idx ON game_commands (game_id, id);
