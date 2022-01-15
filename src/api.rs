use chrono::{Duration, NaiveDateTime, Utc};
use glicko2::{Glicko2Rating, GlickoRating};
use rocket::serde::{json::Json, Serialize};
use rusqlite::{named_params, params, OptionalExtension};

use crate::{
    rater::{self, RatedPlayer},
    website::{self, RatingsDbConn},
};

#[derive(Serialize)]
pub struct Stats {
    game_count: i64,
    player_count: i64,
    next_update_in: String,
    pub last_update: i64,
    pub last_update_string: String,
}

#[get("/api/stats")]
pub async fn stats(conn: RatingsDbConn) -> Json<Stats> {
    Json(stats_inner(&conn).await)
}

pub async fn stats_inner(conn: &RatingsDbConn) -> Stats {
    conn.run(|conn| {
        let game_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))
            .unwrap();
        let player_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM players", [], |r| r.get(0))
            .unwrap();
        let last_update: i64 = conn
            .query_row("SELECT last_update FROM config", [], |r| r.get(0))
            .unwrap();

        let time_to_update =
            Duration::seconds(last_update + rater::RATING_PERIOD - Utc::now().timestamp());

        Stats {
            game_count,
            player_count,
            last_update,
            next_update_in: format!(
                "{:}:{:02}",
                time_to_update.num_hours(),
                time_to_update.num_minutes() - time_to_update.num_hours() * 60
            ),
            last_update_string: format!(
                "{} UTC",
                NaiveDateTime::from_timestamp(last_update, 0).format("%H:%M")
            ),
        }
    })
    .await
}

#[derive(Serialize)]
pub struct RankingPlayer {
    pos: i32,
    id: String,
    character: String,
    name: String,
    game_count: i32,
    rating_value: f64,
    rating_deviation: f64,
}

impl RankingPlayer {
    fn from_db(pos: i32, name: String, rated_player: RatedPlayer) -> Self {
        Self {
            pos,
            name,
            id: format!("{:X}", rated_player.id),
            character: website::CHAR_NAMES[rated_player.char_id as usize]
                .1
                .to_owned(),
            game_count: (rated_player.win_count + rated_player.loss_count) as i32,
            rating_value: GlickoRating::from(rated_player.rating).value.round(),
            rating_deviation: GlickoRating::from(rated_player.rating).deviation.round(),
        }
    }
}
#[get("/api/top/all")]
pub async fn top_all(conn: RatingsDbConn) -> Json<Vec<RankingPlayer>> {
    Json(top_all_inner(&conn).await)
}

pub async fn top_all_inner(conn: &RatingsDbConn) -> Vec<RankingPlayer> {
    conn.run(|c| {
        let mut stmt = c
            .prepare(
                " SELECT * FROM player_ratings 
                 JOIN players ON player_ratings.id=players.id
                 WHERE player_ratings.deviation < ?
                 ORDER BY player_ratings.value - player_ratings.deviation DESC LIMIT 100
                 ",
            )
            .unwrap();
        let mut rows = stmt.query(params![rater::MAX_DEVIATION]).unwrap();

        let mut res = Vec::with_capacity(100);
        let mut i = 1;
        while let Some(row) = rows.next().unwrap() {
            let name = row.get("name").unwrap();
            res.push(RankingPlayer::from_db(i, name, RatedPlayer::from_row(row)));
            i += 1;
        }

        res
    })
    .await
}

#[derive(Serialize)]
pub struct SearchResultPlayer {
    name: String,
    id: String,
    character: String,
    rating_value: f64,
    rating_deviation: f64,
    game_count: i32,
}

#[get("/api/search?<name>")]
pub async fn search(conn: RatingsDbConn, name: String) -> Json<Vec<SearchResultPlayer>> {
    Json(search_inner(&conn, name).await)
}

pub async fn search_inner(conn: &RatingsDbConn, search: String) -> Vec<SearchResultPlayer> {
    conn.run(move |c| {
        info!("Searching for {}", search);
        let mut stmt = c
            .prepare(
                "SELECT * FROM
                    player_names NATURAL JOIN player_ratings
                    WHERE name LIKE ?
                    ORDER BY wins DESC
                    LIMIT 1000
                    ",
            )
            .unwrap();
        let mut rows = stmt.query(params![format!("%{}%", search)]).unwrap();

        let mut res = Vec::new();

        while let Some(row) = rows.next().unwrap() {
            let rating: GlickoRating = Glicko2Rating {
                value: row.get("value").unwrap(),
                deviation: row.get("deviation").unwrap(),
                volatility: 0.0,
            }
            .into();
            res.push(SearchResultPlayer {
                name: row.get("name").unwrap(),
                id: format!("{:X}", row.get::<_, i64>("id").unwrap()),
                character: website::CHAR_NAMES[row.get::<_, usize>("char_id").unwrap()]
                    .1
                    .to_owned(),
                rating_value: rating.value.round(),
                rating_deviation: rating.deviation.round(),
                game_count: row.get::<_, i32>("wins").unwrap()
                    + row.get::<_, i32>("losses").unwrap(),
            });
        }
        res
    })
    .await
}

#[get("/api/top/<char_id>")]
pub async fn top_char(conn: RatingsDbConn, char_id: i64) -> Json<Vec<RankingPlayer>> {
    Json(top_char_inner(&conn, char_id).await)
}

pub async fn top_char_inner(conn: &RatingsDbConn, char_id: i64) -> Vec<RankingPlayer> {
    conn.run(move |c| {
        let mut stmt = c
            .prepare(
                " SELECT * FROM player_ratings 
                 JOIN players ON player_ratings.id=players.id
                 WHERE player_ratings.deviation < ? AND player_ratings.char_id = ?
                 ORDER BY player_ratings.value - player_ratings.deviation DESC LIMIT 100
                 ",
            )
            .unwrap();
        let mut rows = stmt.query(params![rater::MAX_DEVIATION, char_id]).unwrap();

        let mut res = Vec::with_capacity(100);
        let mut i = 1;
        while let Some(row) = rows.next().unwrap() {
            let name = row.get("name").unwrap();
            res.push(RankingPlayer::from_db(i, name, RatedPlayer::from_row(row)));
            i += 1;
        }

        res
    })
    .await
}

#[derive(Serialize)]
pub struct PlayerData {
    name: String,
    other_names: Option<Vec<String>>,
    characters: Vec<PlayerCharacterData>,
}

#[derive(Serialize)]
struct PlayerCharacterData {
    character_name: String,
    rating_value: f64,
    rating_deviation: f64,
    win_rate: f64,
    game_count: i32,
    history: Vec<PlayerSet>,
    recent_games: Vec<PlayerSet>,
    matchups: Vec<PlayerMatchup>,
}

const MATCHUP_MIN_GAMES: f64 = 250.0;

#[derive(Serialize)]
struct PlayerSet {
    timestamp: String,
    own_rating_value: f64,
    own_rating_deviation: f64,
    floor: String,
    opponent_name: String,
    opponent_id: String,
    opponent_character: String,
    opponent_rating_value: f64,
    opponent_rating_deviation: f64,
    expected_outcome_min: f64,
    expected_outcome_max: f64,
    result_wins: i32,
    result_losses: i32,
    result_percent: f64,
}

#[derive(Serialize)]
struct PlayerMatchup {
    character: String,
    game_count: i32,
    win_rate_real: f64,
    win_rate_adjusted: f64,
}

pub async fn get_player_data(conn: &RatingsDbConn, id: i64) -> Option<PlayerData> {
    conn.run(move |conn| {
        if conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM players WHERE id=?)",
                params![id],
                |r| r.get(0),
            )
            .unwrap()
        {
            let name: String = conn
                .query_row("SELECT name FROM players WHERE id=?", params![id], |r| {
                    r.get(0)
                })
                .unwrap();
            let other_names = {
                let mut stmt = conn
                    .prepare("SELECT name FROM player_names WHERE id=?")
                    .unwrap();
                let mut rows = stmt.query(params![id]).unwrap();
                let mut other_names = Vec::new();
                while let Some(row) = rows.next().unwrap() {
                    let other_name: String = row.get(0).unwrap();
                    if other_name != name && !other_names.contains(&other_name) {
                        other_names.push(other_name);
                    }
                }

                other_names
            };

            let mut characters = Vec::new();
            for char_id in 0..website::CHAR_NAMES.len() {
                if let Some((wins, losses, value, deviation)) = conn
                    .query_row(
                        "SELECT wins, losses, value, deviation
                        FROM player_ratings
                        WHERE id=? AND char_id=?",
                        params![id, char_id],
                        |row| {
                            Ok((
                                row.get::<_, i32>(0).unwrap(),
                                row.get::<_, i32>(1).unwrap(),
                                row.get::<_, f64>(2).unwrap(),
                                row.get::<_, f64>(3).unwrap(),
                            ))
                        },
                    )
                    .optional()
                    .unwrap()
                {
                    let character_name = website::CHAR_NAMES[char_id].1.to_owned();

                    let history = {
                        let mut stmt = conn
                            .prepare(
                                "SELECT
                                    timestamp,
                                    value_a AS own_value,
                                    deviation_a AS own_deviation,
                                    game_floor,
                                    name_b AS opponent_name,
                                    id_b AS opponent_id,
                                    char_b AS opponent_character,
                                    value_b AS opponent_value,
                                    deviation_b AS opponent_deviation,
                                    winner
                                FROM games NATURAL JOIN game_ratings
                                WHERE games.id_a= :id AND games.char_a = :char_id
                            
                                UNION

                                SELECT
                                    timestamp,
                                    value_b AS own_value,
                                    deviation_b AS own_deviation,
                                    game_floor,
                                    name_a AS opponent_name,
                                    id_a AS opponent_id,
                                    char_a AS opponent_character,
                                    value_a AS opponent_value,
                                    deviation_a AS opponent_deviation,
                                    winner + 2  as winner
                                FROM games NATURAL JOIN game_ratings
                                WHERE games.id_b = :id AND games.char_b = :char_id

                                ORDER BY timestamp DESC LIMIT 200",
                            )
                            .unwrap();

                        let mut rows = stmt
                            .query(named_params! {":id" : id, ":char_id": char_id})
                            .unwrap();
                        let mut history = Vec::<PlayerSet>::new();
                        while let Some(row) = rows.next().unwrap() {
                            let timestamp: i64 = row.get("timestamp").unwrap();
                            let own_value: f64 = row.get("own_value").unwrap();
                            let own_deviation: f64 = row.get("own_deviation").unwrap();
                            let floor: i64 = row.get("game_floor").unwrap();
                            let opponent_name: String = row.get("opponent_name").unwrap();
                            let opponent_id: i64 = row.get("opponent_id").unwrap();
                            let opponent_character: i64 = row.get("opponent_character").unwrap();
                            let opponent_value: f64 = row.get("opponent_value").unwrap();
                            let opponent_deviation: f64 = row.get("opponent_deviation").unwrap();
                            let winner: i64 = row.get("winner").unwrap();

                            let own_rating: GlickoRating = Glicko2Rating {
                                value: own_value,
                                deviation: own_deviation,
                                volatility: 0.0,
                            }
                            .into();

                            let opponent_rating: GlickoRating = Glicko2Rating {
                                value: opponent_value,
                                deviation: opponent_deviation,
                                volatility: 0.0,
                            }
                            .into();

                            let own_rating_min = (own_value - own_deviation).exp();
                            let own_rating_max = (own_value + own_deviation).exp();
                            let opp_rating_min = (opponent_value - opponent_deviation).exp();
                            let opp_rating_max = (opponent_value + opponent_deviation).exp();

                            let win_min = own_rating_min / (own_rating_min + opp_rating_max);
                            let win_max = own_rating_max / (own_rating_max + opp_rating_min);

                            if let Some(set) = history.last_mut().filter(|set| {
                                set.opponent_id == format!("{:X}", opponent_id)
                                    && set.opponent_character
                                        == website::CHAR_NAMES[opponent_character as usize].1
                            }) {
                                set.timestamp = format!(
                                    "{}",
                                    NaiveDateTime::from_timestamp(timestamp, 0)
                                        .format("%Y-%m-%d %H:%M")
                                );
                                set.own_rating_value = own_rating.value.round();
                                set.own_rating_deviation = own_rating.deviation.round();
                                set.opponent_rating_value = opponent_rating.value.round();
                                set.opponent_rating_deviation = opponent_rating.deviation.round();

                                match winner {
                                    1 | 4 => set.result_wins += 1,
                                    2 | 3 => set.result_losses += 1,
                                    _ => panic!("Bad winner"),
                                }

                                set.result_percent = ((set.result_wins as f64
                                    / (set.result_wins + set.result_losses) as f64)
                                    * 100.0)
                                    .round();
                            } else {
                                history.push(PlayerSet {
                                    timestamp: format!(
                                        "{}",
                                        NaiveDateTime::from_timestamp(timestamp, 0)
                                            .format("%Y-%m-%d %H:%M")
                                    ),
                                    own_rating_value: own_rating.value.round(),
                                    own_rating_deviation: own_rating.deviation.round(),
                                    floor: match floor {
                                        99 => format!("Celestial"),
                                        n => format!("Floor {}", n),
                                    },
                                    opponent_name: opponent_name,
                                    opponent_id: format!("{:X}", opponent_id),
                                    opponent_character: website::CHAR_NAMES
                                        [opponent_character as usize]
                                        .1
                                        .to_owned(),
                                    opponent_rating_value: opponent_rating.value.round(),
                                    opponent_rating_deviation: opponent_rating.deviation.round(),
                                    expected_outcome_min: (win_min * 100.0).round(),
                                    expected_outcome_max: (win_max * 100.0).round(),
                                    result_wins: match winner {
                                        1 | 4 => 1,
                                        _ => 0,
                                    },
                                    result_losses: match winner {
                                        2 | 3 => 1,
                                        _ => 0,
                                    },
                                    result_percent: match winner {
                                        1 | 4 => 100.0,
                                        _ => 0.0,
                                    },
                                });
                            }
                        }

                        history
                    };

                    let recent_games = {
                        let mut stmt = conn
                            .prepare(
                                "SELECT
                            games.timestamp AS timestamp,
                            game_floor,
                            name_b AS opponent_name,
                            games.id_b AS opponent_id,
                            games.char_b AS opponent_character,
                            winner
                        FROM games LEFT JOIN game_ratings 
                        ON games.id_a = game_ratings.id_a 
                            AND games.id_b = game_ratings.id_b 
                            AND games.timestamp = game_ratings.timestamp
                        WHERE games.id_a= :id 
                            AND games.char_a = :char_id 
                            AND game_ratings.id_a IS NULL
                    
                        UNION

                        SELECT
                            games.timestamp AS timestamp,
                            game_floor,
                            name_a AS opponent_name,
                            games.id_a AS opponent_id,
                            games.char_a AS opponent_character,
                            winner + 2  as winner

                        FROM games LEFT JOIN game_ratings 
                        ON games.id_a = game_ratings.id_a 
                            AND games.id_b = game_ratings.id_b 
                            AND games.timestamp = game_ratings.timestamp
                        WHERE games.id_b= :id 
                            AND games.char_b = :char_id 
                            AND game_ratings.id_a IS NULL

                        ORDER BY games.timestamp DESC",
                            )
                            .unwrap();

                        let mut rows = stmt
                            .query(named_params! {":id" : id, ":char_id": char_id})
                            .unwrap();
                        let mut recent_games = Vec::<PlayerSet>::new();
                        while let Some(row) = rows.next().unwrap() {
                            let timestamp: i64 = row.get("timestamp").unwrap();
                            let floor: i64 = row.get("game_floor").unwrap();
                            let opponent_name: String = row.get("opponent_name").unwrap();
                            let opponent_id: i64 = row.get("opponent_id").unwrap();
                            let opponent_character: i64 = row.get("opponent_character").unwrap();
                            let winner: i64 = row.get("winner").unwrap();

                            let own_rating: GlickoRating = Glicko2Rating {
                                value: value,
                                deviation: deviation,
                                volatility: 0.0,
                            }
                            .into();

                            let (opponent_value, opponent_deviation) = conn
                                .query_row(
                                    "SELECT value, deviation
                                FROM player_ratings
                                WHERE id=? AND char_id=?",
                                    params![opponent_id, opponent_character],
                                    |row| {
                                        Ok((
                                            row.get::<_, f64>(0).unwrap(),
                                            row.get::<_, f64>(1).unwrap(),
                                        ))
                                    },
                                )
                                .optional()
                                .unwrap()
                                .unwrap_or((0.0, 350.0 / 173.7178));

                            let opponent_rating: GlickoRating = Glicko2Rating {
                                value: opponent_value,
                                deviation: opponent_deviation,
                                volatility: 0.0,
                            }
                            .into();

                            let own_rating_min = (value - deviation).exp();
                            let own_rating_max = (value + deviation).exp();
                            let opp_rating_min = (opponent_value - opponent_deviation).exp();
                            let opp_rating_max = (opponent_value + opponent_deviation).exp();

                            let win_min = own_rating_min / (own_rating_min + opp_rating_max);
                            let win_max = own_rating_max / (own_rating_max + opp_rating_min);

                            if let Some(set) = recent_games.last_mut().filter(|set| {
                                set.opponent_id == format!("{:X}", opponent_id)
                                    && set.opponent_character
                                        == website::CHAR_NAMES[opponent_character as usize].1
                            }) {
                                set.timestamp = format!(
                                    "{}",
                                    NaiveDateTime::from_timestamp(timestamp, 0)
                                        .format("%Y-%m-%d %H:%M")
                                );
                                set.own_rating_value = own_rating.value.round();
                                set.own_rating_deviation = own_rating.deviation.round();
                                set.opponent_rating_value = opponent_rating.value.round();
                                set.opponent_rating_deviation = opponent_rating.deviation.round();

                                match winner {
                                    1 | 4 => set.result_wins += 1,
                                    2 | 3 => set.result_losses += 1,
                                    _ => panic!("Bad winner"),
                                }

                                set.result_percent = ((set.result_wins as f64
                                    / (set.result_wins + set.result_losses) as f64)
                                    * 100.0)
                                    .round();
                            } else {
                                recent_games.push(PlayerSet {
                                    timestamp: format!(
                                        "{}",
                                        NaiveDateTime::from_timestamp(timestamp, 0)
                                            .format("%Y-%m-%d %H:%M")
                                    ),
                                    own_rating_value: own_rating.value.round(),
                                    own_rating_deviation: own_rating.deviation.round(),
                                    floor: match floor {
                                        99 => format!("Celestial"),
                                        n => format!("Floor {}", n),
                                    },
                                    opponent_name: opponent_name,
                                    opponent_id: format!("{:X}", opponent_id),
                                    opponent_character: website::CHAR_NAMES
                                        [opponent_character as usize]
                                        .1
                                        .to_owned(),
                                    opponent_rating_value: opponent_rating.value.round(),
                                    opponent_rating_deviation: opponent_rating.deviation.round(),
                                    expected_outcome_min: (win_min * 100.0).round(),
                                    expected_outcome_max: (win_max * 100.0).round(),
                                    result_wins: match winner {
                                        1 | 4 => 1,
                                        _ => 0,
                                    },
                                    result_losses: match winner {
                                        2 | 3 => 1,
                                        _ => 0,
                                    },
                                    result_percent: match winner {
                                        1 | 4 => 100.0,
                                        _ => 0.0,
                                    },
                                });
                            }
                        }

                        recent_games
                    };

                    let matchups = {
                        let mut stmt = conn
                            .prepare(
                                "SELECT
                                    opp_char_id,
                                    wins_real,
                                    wins_adjusted,
                                    losses_real,
                                    losses_adjusted
                                FROM player_matchups
                                WHERE id = ?
                                    AND char_id = ?
                                ORDER BY wins_real DESC",
                            )
                            .unwrap();

                        let mut rows = stmt.query(params![id, char_id]).unwrap();
                        let mut matchups = Vec::<PlayerMatchup>::new();
                        while let Some(row) = rows.next().unwrap() {
                            let opp_char_id: usize = row.get(0).unwrap();
                            let wins_real: f64 = row.get(1).unwrap();
                            let wins_adjusted: f64 = row.get(2).unwrap();
                            let losses_real: f64 = row.get(3).unwrap();
                            let losses_adjusted: f64 = row.get(4).unwrap();
                            matchups.push(PlayerMatchup {
                                character: website::CHAR_NAMES[opp_char_id].1.to_owned(),
                                game_count: (wins_real + losses_real) as i32,
                                win_rate_real: (wins_real / (wins_real + losses_real) * 100.0)
                                    .round(),
                                win_rate_adjusted: (wins_adjusted
                                    / (wins_adjusted + losses_adjusted)
                                    * 100.0)
                                    .round(),
                            });
                        }

                        matchups.sort_by_key(|m| -(m.win_rate_adjusted as i32));

                        matchups
                    };

                    characters.push(PlayerCharacterData {
                        character_name,
                        game_count: wins + losses,
                        win_rate: wins as f64 / (wins + losses) as f64,
                        rating_value: (value * 173.7178 + 1500.0).round(),
                        rating_deviation: (deviation * 173.7178).round(),
                        history,
                        recent_games,
                        matchups,
                    });
                }
            }

            characters.sort_by_key(|c| -c.game_count);

            Some(PlayerData {
                name,
                other_names: if other_names.is_empty() {
                    None
                } else {
                    Some(other_names)
                },
                characters,
            })
        } else {
            None
        }
    })
    .await
}

#[derive(Serialize)]
pub struct CharacterMatchups {
    name: String,
    matchups: Vec<Matchup>,
}

#[derive(Serialize)]
pub struct Matchup {
    win_rate_real: f64,
    win_rate_adjusted: f64,
    game_count: i32,
    suspicious: bool,
    evaluation: &'static str,
}

fn get_evaluation(wins: f64, losses: f64, game_count: f64) -> &'static str {
    if game_count < MATCHUP_MIN_GAMES {
        return "none";
    }

    let r = wins / (wins + losses);
    if r > 0.6 {
        "verygood"
    } else if r > 0.56 {
        "good"
    } else if r > 0.52 {
        "slightlygood"
    } else if r > 0.48 {
        "ok"
    } else if r > 0.44 {
        "slightlybad"
    } else if r > 0.40 {
        "bad"
    } else {
        "verybad"
    }
}

pub async fn matchups_global_inner(conn: &RatingsDbConn) -> Vec<CharacterMatchups> {
    conn.run(move |conn| {
        (0..website::CHAR_NAMES.len())
            .map(|char_id| CharacterMatchups {
                name: website::CHAR_NAMES[char_id].1.to_owned(),
                matchups: (0..website::CHAR_NAMES.len())
                    .map(|opp_char_id| {
                        conn.query_row(
                            "SELECT
                                wins_real,
                                wins_adjusted,
                                losses_real,
                                losses_adjusted
                            FROM global_matchups
                            WHERE char_id = ? AND opp_char_id = ?",
                            params![char_id, opp_char_id],
                            |row| {
                                Ok((
                                    row.get::<_, f64>(0).unwrap(),
                                    row.get::<_, f64>(1).unwrap(),
                                    row.get::<_, f64>(2).unwrap(),
                                    row.get::<_, f64>(3).unwrap(),
                                ))
                            },
                        )
                        .optional()
                        .unwrap()
                        .map(
                            |(wins_real, wins_adjusted, losses_real, losses_adjusted)| Matchup {
                                win_rate_real: (wins_real / (wins_real + losses_real) * 100.0)
                                    .round(),
                                win_rate_adjusted: (wins_adjusted
                                    / (wins_adjusted + losses_adjusted)
                                    * 100.0)
                                    .round(),
                                game_count: (wins_real + losses_real) as i32,
                                suspicious: wins_real + losses_real < MATCHUP_MIN_GAMES,
                                evaluation: get_evaluation(
                                    wins_adjusted,
                                    losses_adjusted,
                                    wins_real + losses_real,
                                ),
                            },
                        )
                        .unwrap_or(Matchup {
                            win_rate_real: f64::NAN,
                            win_rate_adjusted: f64::NAN,
                            game_count: 0,
                            suspicious: true,
                            evaluation: "none",
                        })
                    })
                    .collect(),
            })
            .collect()
    })
    .await
}

pub async fn matchups_high_rated_inner(conn: &RatingsDbConn) -> Vec<CharacterMatchups> {
    conn.run(move |conn| {
        (0..website::CHAR_NAMES.len())
            .map(|char_id| CharacterMatchups {
                name: website::CHAR_NAMES[char_id].1.to_owned(),
                matchups: (0..website::CHAR_NAMES.len())
                    .map(|opp_char_id| {
                        conn.query_row(
                            "SELECT
                                wins_real,
                                wins_adjusted,
                                losses_real,
                                losses_adjusted
                            FROM high_rated_matchups
                            WHERE char_id = ? AND opp_char_id = ?",
                            params![char_id, opp_char_id],
                            |row| {
                                Ok((
                                    row.get::<_, f64>(0).unwrap(),
                                    row.get::<_, f64>(1).unwrap(),
                                    row.get::<_, f64>(2).unwrap(),
                                    row.get::<_, f64>(3).unwrap(),
                                ))
                            },
                        )
                        .optional()
                        .unwrap()
                        .map(
                            |(wins_real, wins_adjusted, losses_real, losses_adjusted)| Matchup {
                                win_rate_real: (wins_real / (wins_real + losses_real) * 100.0)
                                    .round(),
                                win_rate_adjusted: (wins_adjusted
                                    / (wins_adjusted + losses_adjusted)
                                    * 100.0)
                                    .round(),
                                game_count: (wins_real + losses_real) as i32,
                                suspicious: wins_real + losses_real < MATCHUP_MIN_GAMES,
                                evaluation: get_evaluation(
                                    wins_adjusted,
                                    losses_adjusted,
                                    wins_real + losses_real,
                                ),
                            },
                        )
                        .unwrap_or(Matchup {
                            win_rate_real: f64::NAN,
                            win_rate_adjusted: f64::NAN,
                            game_count: 0,
                            suspicious: true,
                            evaluation: "none",
                        })
                    })
                    .collect(),
            })
            .collect()
    })
    .await
}

#[derive(Serialize)]
pub struct VersusCharacterMatchups {
    name: String,
    matchups: Vec<VersusMatchup>,
}

#[derive(Serialize)]
pub struct VersusMatchup {
    win_rate: f64,
    game_count: i32,
    pair_count: i32,
    suspicious: bool,
    evaluation: &'static str,
}

pub async fn matchups_versus(conn: &RatingsDbConn) -> Vec<VersusCharacterMatchups> {
    conn.run(move |conn| {
        (0..website::CHAR_NAMES.len())
            .map(|char_id| VersusCharacterMatchups {
                name: website::CHAR_NAMES[char_id].1.to_owned(),
                matchups: (0..website::CHAR_NAMES.len())
                    .map(|opp_char_id| {
                        if char_id == opp_char_id {
                            VersusMatchup {
                                win_rate: 50.0,
                                game_count: 0,
                                pair_count: 0,
                                suspicious: false,
                                evaluation: "ok",
                            }
                        } else {
                            conn.query_row(
                                "SELECT win_rate, game_count, pair_count
                                FROM versus_matchups
                                WHERE char_a = ? AND char_b = ?",
                                params![char_id, opp_char_id],
                                |row| {
                                    Ok((
                                        row.get::<_, f64>(0)?,
                                        row.get::<_, i32>(1)?,
                                        row.get::<_, i32>(2)?,
                                    ))
                                },
                            )
                            .optional()
                            .unwrap()
                            .map(|(win_rate, game_count, pair_count)| VersusMatchup {
                                win_rate: (win_rate * 100.0).round(),
                                game_count,
                                pair_count,
                                suspicious: pair_count < 50 || game_count < 250,
                                evaluation: get_evaluation(win_rate, 1.0 - win_rate, f64::INFINITY),
                            })
                            .unwrap_or(VersusMatchup {
                                win_rate: f64::NAN,
                                game_count: 0,
                                pair_count: 0,
                                suspicious: true,
                                evaluation: "none",
                            })
                        }
                    })
                    .collect(),
            })
            .collect()
    })
    .await
}

#[derive(Serialize)]
pub struct FloorPlayers {
    floor: String,
    player_count: i64,
    player_percentage: f64,
    game_count: i64,
    game_percentage: f64,
}

pub async fn player_floors_distribution(conn: &RatingsDbConn) -> Vec<FloorPlayers> {
    conn.run(move |conn| {
        let total_players: i64 = conn
            .query_row("SELECT COUNT(*) FROM players", [], |r| r.get(0))
            .unwrap();

        let total_games: i64 = conn
            .query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))
            .unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT
                floor, player_count, game_count
                FROM player_floor_distribution
                ORDER BY floor ASC",
            )
            .unwrap();

        let mut rows = stmt.query([]).unwrap();

        let mut res = Vec::<FloorPlayers>::new();
        while let Some(row) = rows.next().unwrap() {
            let floor: i64 = row.get(0).unwrap();
            let player_count: i64 = row.get(1).unwrap();
            let game_count: i64 = row.get(2).unwrap();

            res.push(FloorPlayers {
                floor: match floor {
                    99 => format!("Celestial"),
                    n => format!("Floor {}", n),
                },
                player_count,
                player_percentage: (1000.0 * player_count as f64 / total_players as f64).round()
                    / 10.0,
                game_count,
                game_percentage: (1000.0 * game_count as f64 / total_games as f64).round() / 10.0,
            });
        }

        res
    })
    .await
}

#[derive(Serialize)]
pub struct RatingPlayers {
    min_rating: i64,
    max_rating: i64,
    player_count: i64,
    player_percentage: f64,
    player_count_cum: i64,
    player_percentage_cum: f64,
}

pub async fn player_ratings_distribution(conn: &RatingsDbConn) -> Vec<RatingPlayers> {
    conn.run(move |conn| {
        let total_players: i64 = conn
            .query_row(
                "
        SELECT COUNT(*)
        FROM player_ratings
        WHERE deviation < ?",
                params![rater::MAX_DEVIATION],
                |r| r.get(0),
            )
            .unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT
                min_rating, max_rating, player_count, player_count_cum
                FROM player_rating_distribution
                ORDER BY min_rating ASC",
            )
            .unwrap();

        let mut rows = stmt.query([]).unwrap();

        let mut res = Vec::<RatingPlayers>::new();
        while let Some(row) = rows.next().unwrap() {
            let min_rating: i64 = row.get(0).unwrap();
            let max_rating: i64 = row.get(1).unwrap();
            let player_count: i64 = row.get(2).unwrap();
            let player_count_cum: i64 = row.get(3).unwrap();

            res.push(RatingPlayers {
                min_rating,
                max_rating,
                player_count,
                player_percentage: (1000.0 * player_count as f64 / total_players as f64).round()
                    / 10.0,
                player_count_cum,
                player_percentage_cum: (1000.0 * player_count_cum as f64 / total_players as f64)
                    .round()
                    / 10.0,
            });
        }

        res
    })
    .await
}

#[get("/api/outcomes")]
pub async fn outcomes(conn: RatingsDbConn) -> Json<Vec<f64>> {
    Json(
        conn.run(move |conn| {
            let mut outcomes = vec![(0.0, 0.0); 101];

            let mut stmt = conn
                .prepare(
                    "SELECT
                value_a, value_b, winner
                FROM games NATURAL JOIN game_ratings
                WHERE deviation_a < ? AND deviation_b < ?;",
                )
                .unwrap();

            let mut rows = stmt
                .query(params![rater::MAX_DEVIATION, rater::MAX_DEVIATION])
                .unwrap();
            while let Some(row) = rows.next().unwrap() {
                let a: f64 = row.get(0).unwrap();
                let b: f64 = row.get(1).unwrap();
                let winner: i64 = row.get(2).unwrap();

                let p = a.exp() / (a.exp() + b.exp());

                let o = outcomes.get_mut((p * 100.0).round() as usize).unwrap();
                if winner == 1 {
                    o.0 += 1.0;
                }
                o.1 += 1.0;
            }

            outcomes
                .into_iter()
                .map(|(wins, total)| wins / total)
                .collect()
        })
        .await,
    )
}
