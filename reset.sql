DELETE FROM game_ratings;
DELETE FROM players;
DELETE FROM player_ratings;
DELETE FROM daily_ratings;
DELETE FROM player_matchups;
DELETE FROM global_matchups;
DELETE FROM top_1000_matchups;
DELETE FROM proportional_matchups;
DELETE FROM top_100_matchups;
DELETE FROM player_names;
DELETE FROM ranking_character;
DELETE FROM ranking_global;
DELETE FROM player_rating_distribution;
DELETE FROM player_floor_distribution;

DELETE FROM config;
INSERT INTO config VALUES(1635717600);
