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

CREATE TABLE vip_status (
    id INTEGER NOT NULL,
    vip_status TEXT NOT NULL,
    notes TEXT NOT NULL,
    PRIMARY KEY(id)
);


CREATE TABLE config (
    last_update INTEGER NOT NULL
);

INSERT INTO config VALUES(1635717600);
