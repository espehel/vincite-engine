-- The original game_events had no game, time or payload, so it could not
-- record anything. Replace it with events that are due at a tick and are
-- applied by the ticker, or by a read that reaches that tick first.
DROP TABLE game_events;

CREATE TABLE game_events (
    -- Breaks ties between events due at the same tick, so they always apply
    -- in the same order.
    id BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    game_id UUID NOT NULL REFERENCES games (id) ON DELETE CASCADE,
    -- The command that caused the event, or NULL when another event did.
    command_id BIGINT REFERENCES game_commands (id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    payload JSONB NOT NULL,
    -- due_tick is what the rules use; due_at is the same moment as wall-clock
    -- time, so the ticker can find due events without knowing each game's start.
    due_tick BIGINT NOT NULL CHECK (due_tick >= 0),
    due_at TIMESTAMPTZ NOT NULL,
    processed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- The ticker's poll: the earliest pending events across all games.
CREATE INDEX game_events_pending_due_at_idx
    ON game_events (due_at) WHERE processed_at IS NULL;

-- Loading one game's pending events, in the order they apply.
CREATE INDEX game_events_pending_game_idx
    ON game_events (game_id, due_tick, id) WHERE processed_at IS NULL;

CREATE INDEX game_events_command_id_idx ON game_events (command_id);
