-- Ticks count from when the game started, not from when it was created, so a
-- game that sat open in the lobby does not start with production already
-- accrued. NULL while the game is open.
ALTER TABLE games
ADD COLUMN started_at TIMESTAMPTZ;

-- Games that are already running have been counting ticks from created_at, so
-- keep that as their start rather than moving their clock.
UPDATE games
SET started_at = created_at
WHERE status = 'Running';
