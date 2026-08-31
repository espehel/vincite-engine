-- Add up migration script here
ALTER TABLE games
ADD COLUMN maximum_players SMALLINT NOT NULL
    CONSTRAINT games_maximum_players_u8_range
    CHECK (maximum_players BETWEEN 0 AND 255);
