DELETE FROM game_ratings;
DELETE FROM players;
DELETE FROM player_ratings;
DELETE FROM player_matchups;
DELETE FROM global_matchups;
DELETE FROM high_rated_matchups;
DELETE FROM config;

DROP TABLE player_names;
CREATE VIRTUAL TABLE player_names USING fts5(
    id,
    name,
);


INSERT INTO config VALUES(1635717600);
