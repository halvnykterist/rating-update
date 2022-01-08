use chrono::NaiveDateTime;
use ggst_api::Character;
use glicko2::{Glicko2Rating, GlickoRating};
use rocket::serde::{json::Json, Serialize};
use rusqlite::{named_params, params, Connection, OptionalExtension};

use crate::{
    rater::{self, RatedPlayer},
    website::{self, RatingsDbConn},
};

#[derive(Serialize)]
pub struct Stats {
    game_count: i64,
    player_count: i64,
    pub last_update: i64,
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

        Stats {
            game_count,
            player_count,
            last_update,
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
                 ORDER BY player_ratings.value DESC LIMIT 100
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
                 ORDER BY player_ratings.value DESC LIMIT 100
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
    //win_rate: f64,
    game_count: i32,
    history: Vec<PlayerSet>,
    //matchups: Vec<PlayerMatchup>,
}

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
    //expected_outcome_min: f64,
    //expected_outcome_max: f64,
    result_wins: i32,
    result_losses: i32,
    //result_percent: f64,
}
//
//#[derive(Serialize)]
//struct PlayerMatchup {
//    character: String,
//    game_count: i32,
//    win_rate_real: f64,
//    win_rated_adjusted: f64,
//}

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
            let last_timestamp: i64 = conn
                .query_row("SELECT last_update FROM config", [], |r| r.get(0))
                .unwrap();

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

                        ORDER BY timestamp DESC LIMIT 1000",
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
                                    result_wins: match winner {
                                        1 | 4 => 1,
                                        _ => 0,
                                    },
                                    result_losses: match winner {
                                        2 | 3 => 1,
                                        _ => 0,
                                    },
                                });
                            }

                            //history.push(PlayerSet {
                            //    timestamp: format!(
                            //        "{}",
                            //        NaiveDateTime::from_timestamp(timestamp, 0)
                            //            .format("%Y-%m-%d %H:%M")
                            //    ),
                            //    opponent_name,
                            //    opponent_id: format!("{:X}", opponent_id),
                            //});
                        }

                        history
                    };

                    characters.push(PlayerCharacterData {
                        character_name,
                        game_count: wins + losses,
                        rating_value: (value * 173.7178 + 1500.0).round(),
                        rating_deviation: (deviation * 173.7178).round(),
                        history,
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

        //grab the name
        //grab the other names
        //iterate over characters
        //  set name
        //  grab rating value/deviation
        //  grab
    })
    .await
}
