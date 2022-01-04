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
    name TEXT NOT NULL
);

CREATE TABLE player_names (
    id INTEGER NOT NULL,
    name TEXT NOT NULL,
    PRIMARY KEY (id, name)
);

CREATE TABLE player_ratings (
    id INTEGER NOT NULL,
    char_id INTEGER NOT NULL,
    game_count INTEGER NOT NULL,
    value REAL NOT NULL,
    deviation REAL NOT NULL,
    volatility REAL NOT NULL,
    PRIMARY KEY(id, char_id)
);

CREATE TABLE player_matchup (
    id INTEGER NOT NULL,
    char_id INTEGER NOT NULL,
    opp_char_id INTEGER NOT NULL,
    wins_real REAL NOT NULL,
    wins_adjusted REAL NOT NULL,
    losses REAL NOT NULL,
    losses_adjusted REAL NOT NULL,
    PRIMARY KEY(id, char_id, opp_char_id)
);

CREATE TABLE player_history (
    id INTEGER NOT NULL,
    char_id INTEGER NOT NULL,
    timestamp INTEGER NOT NULL,
    opp_id INTEGER NOT NULL,
    wins_real REAL NOT NULL,
    wins_adjusted REAL NOT NULL,
    losses REAL NOT NULL,
    losses_adjusted REAL NOT NULL,
    PRIMARY KEY(id, char_id, timestamp)
);
