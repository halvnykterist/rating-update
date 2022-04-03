CREATE TABLE games (
    timestamp INTEGER NOT NULL,
    id_a INTEGER NOT NULL,
    name_a TEXT NOT NULL,
    char_a INTEGER NOT NULL,
    id_b INTEGER NOT NULL,
    name_b TEXT NOT NULL,
    char_b INTEGER NOT NULL,
    winner INTEGER NOT NULL,
    game_floor INTEGER NOT NULL,
    PRIMARY KEY (timestamp, id_a, id_b)
);

CREATE INDEX games_char_a ON games(char_a);
CREATE INDEX games_char_b ON games(char_b);

-- Indices for speeding up player character match history lookup
CREATE INDEX games_id_char_a ON games (
	id_a,
	char_a
)
CREATE INDEX games_id_char_b ON games (
	id_b,
	char_b
)

CREATE TABLE game_ratings (
    timestamp INTEGER NOT NULL,
    id_a INTEGER NOT NULL,
    value_a REAL NOT NULL,
    deviation_a REAL NOT NULL,
    id_b INTEGER NOT NULL,
    value_b REAL NOT NULL,
    deviation_b REAL NOT NULL,
    PRIMARY KEY (timestamp, id_a, id_b)
);

CREATE INDEX ratings_value_a ON game_ratings(value_a);
CREATE INDEX ratings_value_b ON game_ratings(value_b);
CREATE INDEX ratings_dev_a ON game_ratings(deviation_a);
CREATE INDEX ratings_dev_b ON game_ratings(deviation_b);

CREATE TABLE players  (
    id INTEGER NOT NULL PRIMARY KEY,
    floor INTEGER NOT NULL,
    name TEXT NOT NULL
);

CREATE TABLE player_names (
    id INTEGER NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY(id, name)
);

CREATE TABLE player_ratings (
    id INTEGER NOT NULL,
    char_id INTEGER NOT NULL,
    wins INTEGER NOT NULL,
    losses INTEGER NOT NULL,
    value REAL NOT NULL,
    deviation REAL NOT NULL,
    volatility REAL NOT NULL,
    PRIMARY KEY(id, char_id)
);

CREATE INDEX player_value ON player_ratings(value);
CREATE INDEX player_dev ON player_ratings(deviation);

CREATE TABLE player_matchups (
    id INTEGER NOT NULL,
    char_id INTEGER NOT NULL,
    opp_char_id INTEGER NOT NULL,
    wins_real REAL NOT NULL,
    wins_adjusted REAL NOT NULL,
    losses_real REAL NOT NULL,
    losses_adjusted REAL NOT NULL,
    PRIMARY KEY(id, char_id, opp_char_id)
);

CREATE TABLE global_matchups(
    char_id INTEGER NOT NULL,
    opp_char_id INTEGER NOT NULL,
    wins_real REAL NOT NULL,
    wins_adjusted REAL NOT NULL,
    losses_real REAL NOT NULL,
    losses_adjusted REAL NOT NULL,
    PRIMARY KEY(char_id, opp_char_id)
);

CREATE TABLE high_rated_matchups(
    char_id INTEGER NOT NULL,
    opp_char_id INTEGER NOT NULL,
    wins_real REAL NOT NULL,
    wins_adjusted REAL NOT NULL,
    losses_real REAL NOT NULL,
    losses_adjusted REAL NOT NULL,
    PRIMARY KEY(char_id, opp_char_id)
);

CREATE TABLE versus_matchups(
    char_a INTEGER NOT NULL,
    char_b INTEGER NOT NULL,
    game_count INTEGER NOT NULL,
    pair_count INTEGER NOT NULL,
    win_rate REAL NOT NULL,
    PRIMARY KEY(char_a, char_b)
);

CREATE TABLE player_floor_distribution(
    floor INTEGER NOT NULL,
    player_count INTEGER NOT NULL,
    game_count INTEGER NOT NULL,
    PRIMARY KEY(floor)
);

CREATE TABLE player_rating_distribution(
    min_rating INTEGER NOT NULL,
    max_rating INTEGER NOT NULL,
    player_count INTEGER NOT NULL,
    player_count_cum INTEGER NOT NULL,
    PRIMARY KEY(min_rating, max_rating)
);

CREATE TABLE ranking_global (
    global_rank INTEGER NOT NULL,
    id INTEGER NOT NULL,
    char_id INTEGER NOT NULL,
    PRIMARY KEY(global_rank)
);

CREATE TABLE ranking_character (
    character_rank INTEGER NOT NULL,
    char_id INTEGER NOT NULL,
    id INTEGER NOT NULL,
    PRIMARY KEY(character_rank, char_id)
);

CREATE TABLE character_popularity_global (
    char_id INTEGER NOT NULL,
    popularity REAL NOT NULL,
    PRIMARY KEY(char_id)
);

CREATE TABLE character_popularity_rating (
    char_id INTEGER NOT NULL,
    rating_bracket INTEGER NOT NULL,
    popularity REAL NOT NULL,
    PRIMARY KEY(char_id, rating_bracket)
);

CREATE TABLE fraud_index (
    char_id INTEGER NOT NULL,
    player_count INTEGER NOT NULL,
    avg_delta REAL NOT NULL,
    PRIMARY KEY(char_id)
);

CREATE TABLE fraud_index_higher_rated (
    char_id INTEGER NOT NULL,
    player_count INTEGER NOT NULL,
    avg_delta REAL NOT NULL,
    PRIMARY KEY(char_id)
);

CREATE TABLE fraud_index_highest_rated (
    char_id INTEGER NOT NULL,
    player_count INTEGER NOT NULL,
    avg_delta REAL NOT NULL,
    PRIMARY KEY(char_id)
);

CREATE TABLE vip_status (
    id INTEGER NOT NULL,
    vip_status TEXT NOT NULL,
    notes TEXT NOT NULL,
    PRIMARY KEY(id)
);

CREATE TABLE cheater_status (
    id INTEGER NOT NULL,
    cheater_status TEXT NOT NULL,
    notes TEXT NOT NULL,
    PRIMARY KEY(id)
);


CREATE TABLE config (
    last_update INTEGER NOT NULL
);

CREATE TABLE hits (
    page TEXT NOT NULL,
    hit_count INTEGER NOT NULL,
    PRIMARY KEY(page)
);

INSERT INTO config VALUES(1635717600);
