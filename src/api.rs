use crate::{ggst_api};
use chrono::{Duration, NaiveDateTime, Utc};
use fxhash::FxHashMap;
use rand::distributions::{Alphanumeric, DistString};
use rocket::serde::{json::Json, Serialize};
use rusqlite::{named_params, params, Connection, OptionalExtension};

use crate::{
    glicko,
    glicko::Rating,
    rater::{self, RatedPlayer},
    website::{self, RatingsDbConn},
};

type Result<T> = std::result::Result<T, anyhow::Error>;

#[derive(Serialize)]
pub struct Stats {
    game_count: i64,
    player_count: i64,
    activity_7d: Activity,
    activity_24h: Activity,
    activity_1h: Activity,
}

#[derive(Serialize)]
pub struct Activity {
    players: i64,
    games: i64,
    over_1700: i64,
    over_1900: i64,
    over_2100: i64,
    sub_1300: i64,
    sub_1100: i64,
    sub_900: i64,
}

fn to_platform_string(i: i64) -> &'static str {
    match i {
        1 => "PS",
        2 => "XB",
        3 => "PC",
        _ => "??",
    }
}

impl Activity {
    fn calculate(conn: &mut Connection, time_offset: i64) -> Self {
        let t = Utc::now().timestamp() - time_offset;
        let players = conn
            .query_row(
                "SELECT COUNT(DISTINCT(id)) FROM
                    (SELECT id_a as id FROM game_ratings WHERE timestamp > ?
                        UNION
                    SELECT id_b as id FROM game_ratings WHERE timestamp > ?)",
                params![t, t],
                |r| r.get(0),
            )
            .unwrap();

        let games = conn
            .query_row(
                "SELECT COUNT(*) FROM game_ratings WHERE timestamp > ?",
                params![t],
                |r| r.get(0),
            )
            .unwrap();

        let over_1700 = conn
            .query_row(
                "SELECT COUNT(*) FROM game_ratings WHERE timestamp > ?
                    AND (value_a > 1700 OR value_b > 1700)",
                params![t],
                |r| r.get(0),
            )
            .unwrap();

        let over_1900 = conn
            .query_row(
                "SELECT COUNT(*) FROM game_ratings WHERE timestamp > ?
                    AND (value_a > 1900 OR value_b > 1900)",
                params![t],
                |r| r.get(0),
            )
            .unwrap();

        let over_2100 = conn
            .query_row(
                "SELECT COUNT(*) FROM game_ratings WHERE timestamp > ?
                    AND (value_a > 2100 OR value_b > 2100)",
                params![t],
                |r| r.get(0),
            )
            .unwrap();

        let sub_1300 = conn
            .query_row(
                "SELECT COUNT(*) FROM game_ratings WHERE timestamp > ?
                    AND (value_a < 1200 OR value_b < 1300)",
                params![t],
                |r| r.get(0),
            )
            .unwrap();
        let sub_1100 = conn
            .query_row(
                "SELECT COUNT(*) FROM game_ratings WHERE timestamp > ?
                    AND (value_a < 1100 OR value_b < 1100)",
                params![t],
                |r| r.get(0),
            )
            .unwrap();
        let sub_900 = conn
            .query_row(
                "SELECT COUNT(*) FROM game_ratings WHERE timestamp > ?
                    AND (value_a < 900 OR value_b < 900)",
                params![t],
                |r| r.get(0),
            )
            .unwrap();

        Self {
            players,
            games,
            over_1700,
            over_1900,
            over_2100,
            sub_1300,
            sub_1100,
            sub_900,
        }
    }
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

        let activity_7d = Activity::calculate(conn, 60 * 60 * 24 * 7);
        let activity_24h = Activity::calculate(conn, 60 * 60 * 24);
        let activity_1h = Activity::calculate(conn, 60 * 60);

        Stats {
            game_count,
            player_count,
            activity_7d,
            activity_24h,
            activity_1h,
        }
    })
    .await
}

#[get("/api/daily_games?<length>")]
pub async fn daily_games(
    conn: RatingsDbConn,
    length: Option<i64>,
) -> Json<(Vec<String>, Vec<i64>, Vec<i64>)> {
    Json(
        conn.run(move |conn| {
            let tx = conn.transaction().unwrap();
            let now = NaiveDateTime::from_timestamp_opt(Utc::now().timestamp(), 0).unwrap();
            let now_date = now.date();
            let length = length.unwrap_or(60);
            let then = now_date - Duration::days(length);

            (
                then.iter_days()
                    .take(length as usize)
                    .map(|date| date.format("%Y-%m-%d").to_string())
                    .collect(),
                then.iter_days()
                    .take(length as usize)
                    .map(|date| {
                        let from = date.and_hms_opt(0, 0, 0).unwrap().timestamp();
                        let to = from + 24 * 60 * 60;
                        tx.query_row(
                            "select count(*) from games where timestamp > ? and timestamp < ?",
                            params![from, to],
                            |r| r.get(0),
                        )
                        .unwrap()
                    })
                    .collect(),
                then.iter_days()
                    .take(length as usize)
                    .map(|date| {
                        let from = date.and_hms_opt(0, 0, 0).unwrap().timestamp();
                        let to = from + 24 * 60 * 60;
                        tx.query_row(
                            "select count(distinct id) from (
                                select id_a as id from games where timestamp > ? and timestamp < ?
                                union
                                select id_b as id from games where timestamp > ? and timestamp < ?
                            )
                            ",
                            params![from, to, from, to],
                            |r| r.get(0),
                        )
                        .unwrap()
                    })
                    .collect(),
            )
        })
        .await,
    )
}

#[get("/api/weekly_games?<length>")]
pub async fn weekly_games(
    conn: RatingsDbConn,
    length: Option<i64>,
) -> Json<(Vec<String>, Vec<i64>, Vec<i64>)> {
    Json(
        conn.run(move |conn| {
            let tx = conn.transaction().unwrap();
            let now = NaiveDateTime::from_timestamp_opt(Utc::now().timestamp(), 0).unwrap();
            let now_date = now.date();
            let length = length.unwrap_or(8);
            let then = now_date - Duration::weeks(length);

            (
                then.iter_days()
                    .take(length as usize)
                    .map(|date| date.format("%Y-%m-%d").to_string())
                    .collect(),
                then.iter_days()
                    .take(length as usize)
                    .map(|date| {
                        let from = date.and_hms_opt(0, 0, 0).unwrap().timestamp();
                        let to = from + 7 * 24 * 60 * 60;
                        tx.query_row(
                            "select count(*) from games where timestamp > ? and timestamp < ?",
                            params![from, to],
                            |r| r.get(0),
                        )
                        .unwrap()
                    })
                    .collect(),
                then.iter_days()
                    .take(length as usize)
                    .map(|date| {
                        let from = date.and_hms_opt(0, 0, 0).unwrap().timestamp();
                        let to = from + 7 * 24 * 60 * 60;
                        tx.query_row(
                            "select count(distinct id) from (
                                select id_a as id from games where timestamp > ? and timestamp < ?
                                union
                                select id_b as id from games where timestamp > ? and timestamp < ?
                            )
                            ",
                            params![from, to, from, to],
                            |r| r.get(0),
                        )
                        .unwrap()
                    })
                    .collect(),
            )
        })
        .await,
    )
}

#[get("/api/daily_character_games?<length>")]
pub async fn daily_character_games(
    conn: RatingsDbConn,
    length: Option<i64>,
) -> Json<(Vec<String>, Vec<String>, Vec<Vec<i64>>)> {
    Json(
        conn.run(move |conn| {
            let tx = conn.transaction().unwrap();
            let now = NaiveDateTime::from_timestamp_opt(Utc::now().timestamp(), 0).unwrap();
            let now_date = now.date();
            let length = length.unwrap_or(60);
            let then = now_date - Duration::days(length);

            (
                then.iter_days()
                    .take(length as usize)
                    .map(|date| date.format("%Y-%m-%d").to_string())
                    .collect(),
                website::CHAR_NAMES.iter().map(|c| c.0.to_owned()).collect(),
                (0..website::CHAR_NAMES.len())
                    .map(|c| {
                        then.iter_days()
                            .take(length as usize)
                            .map(|date| {
                                let from = date.and_hms_opt(0, 0, 0).unwrap().timestamp();
                                let to = from + 24 * 60 * 60;
                                tx.query_row(
                                    "SELECT COUNT(*) 
                                    FROM games 
                                    WHERE timestamp > ? ANd timestamp < ? AND 
                                    (char_a = ? OR char_b = ?)",
                                    params![from, to, c, c],
                                    |r| r.get::<_, i64>(0),
                                )
                                .unwrap()
                            })
                            .collect()
                    })
                    .collect(),
            )
        })
        .await,
    )
}
pub async fn add_hit(_conn: &RatingsDbConn, _page: String) {
    //TODO figure out a way of implementing this that doesn't cause more DB pressure.

    //conn.run(move |conn| {
    //    conn.execute("INSERT OR IGNORE INTO hits VALUES(?, 0)", params![&page])
    //        .unwrap();
    //    conn.execute(
    //        "UPDATE hits SET hit_count = hit_count + 1 WHERE page = ?",
    //        params![&page],
    //    )
    //    .unwrap();
    //})
    //.await;
}

#[derive(Serialize)]
pub struct RankingPlayer {
    pos: i32,
    id: String,
    platform: &'static str,
    character: String,
    character_short: String,
    name: String,
    game_count: i32,
    rating_value: i64,
    rating_deviation: i64,
    vip_status: Option<String>,
    cheater_status: Option<String>,
    hidden_status: Option<String>,
}

impl RankingPlayer {
    fn from_db(
        pos: i32,
        name: String,
        platform: i64,
        vip_status: Option<String>,
        cheater_status: Option<String>,
        hidden_status: Option<String>,
        rated_player: RatedPlayer,
    ) -> Self {
        Self {
            pos,
            name,
            platform: to_platform_string(platform),
            id: format!("{:X}", rated_player.id),
            character: website::CHAR_NAMES[rated_player.char_id as usize]
                .1
                .to_owned(),
            character_short: website::CHAR_NAMES[rated_player.char_id as usize]
                .0
                .to_owned(),
            game_count: (rated_player.win_count + rated_player.loss_count) as i32,
            rating_value: rated_player.rating.value.round() as i64,
            rating_deviation: (rated_player.rating.deviation * 2.0).round() as i64,
            vip_status,
            cheater_status,
            hidden_status,
        }
    }
}
#[get("/api/top/all")]
pub async fn top_all(conn: RatingsDbConn) -> Json<Vec<RankingPlayer>> {
    Json(top_all_inner(&conn).await)
}

#[get("/api/player_rating/<player>")]
pub async fn player_rating_all(conn: RatingsDbConn, player: &str) -> Json<Vec<Rating>> {
    let id = i64::from_str_radix(&player, 16).unwrap();
    let mut res = vec![Rating::default(); website::CHAR_NAMES.len()];
    Json(
        conn.run(move |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT char_id, value, deviation 
        FROM player_ratings
        WHERE id = ?",
                )
                .unwrap();

            let mut rows = stmt.query(params![id]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                let char_id: usize = row.get(0).unwrap();
                res[char_id] = Rating::new(row.get(1).unwrap(), row.get(2).unwrap());
            }

            res
        })
        .await,
    )

    //for char_id in 0..website::CHAR_NAMES.len() {
    //    let conn.run(move |conn| {
    //        conn
    //            .query_row(
    //                "SELECT value, deviation
    //                            FROM player_ratings
    //                            WHERE id=? AND char_id=?",
    //                params![id, char_id],
    //                |r| Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?)),
    //            )
    //            .optional()
    //}
}

#[get("/api/player_rating/<player>/<character_short>")]
pub async fn player_rating(
    conn: RatingsDbConn,
    player: &str,
    character_short: &str,
) -> Option<Json<Rating>> {
    let id = i64::from_str_radix(&player, 16).unwrap();
    if let Some(char_id) = website::CHAR_NAMES
        .iter()
        .position(|(c, _)| *c == character_short)
    {
        conn.run(move |conn| {
            if let Some((value, deviation)) = conn
                .query_row(
                    "SELECT value, deviation
                                FROM player_ratings
                                WHERE id=? AND char_id=?",
                    params![id, char_id],
                    |r| Ok((r.get::<_, f64>(0)?, r.get::<_, f64>(1)?)),
                )
                .optional()
                .unwrap()
            {
                Some(Json(Rating { value, deviation }))
            } else {
                None
            }
        })
        .await
    } else {
        None
    }
}

#[get("/api/accuracy/<player>/<character_short>")]
pub async fn player_rating_accuracy(
    conn: RatingsDbConn,
    player: &str,
    character_short: &str,
) -> Option<Json<Vec<f64>>> {
    let id = i64::from_str_radix(&player, 16).unwrap();
    if let Some(char_id) = website::CHAR_NAMES
        .iter()
        .position(|(c, _)| *c == character_short)
    {
        conn.run(move |conn| {
            let mut buckets = vec![(0.0, 0.0); 11];

            let mut stmt = conn
                .prepare(
                    "
                SELECT
                    value_a as own_value,
                    deviation_a as own_deviation,
                    value_b as opp_value,
                    deviation_b as opponent_deviation,
                    winner
                FROM games NATURAL JOIN game_ratings
                WHERE 
                    games.id_a = :id 
                    AND games.char_a = :char_id 
                    AND game_ratings.deviation_a< 75.0
                    AND game_ratings.deviation_b < 75.0

                UNION

                SELECT
                    value_b as own_value,
                    deviation_b as own_deviation,
                    value_a as opp_value,
                    deviation_a as opponent_deviation,
                    winner + 2 as winner
                FROM games NATURAL JOIN game_ratings
                WHERE 
                    games.id_b = :id 
                    AND games.char_b = :char_id 
                    AND game_ratings.deviation_a < 0.5 
                    AND game_ratings.deviation_b < 0.5",
                )
                .unwrap();

            let mut rows = stmt
                .query(named_params! {
                    ":id" : id,
                    ":char_id": char_id,
                })
                .unwrap();

            while let Some(row) = rows.next().unwrap() {
                let own_rating = Rating::new(row.get(0).unwrap(), row.get(1).unwrap());
                let opp_rating = Rating::new(row.get(2).unwrap(), row.get(3).unwrap());
                let winner: i64 = row.get(4).unwrap();

                let expected = Rating::expected(own_rating, opp_rating);

                let bucket = (expected.min(1.0).max(0.0) * 10.0).round() as usize;

                match winner {
                    1 | 4 => buckets[bucket].0 += 1.0,
                    2 | 3 => buckets[bucket].1 += 1.0,
                    _ => panic!("Bad winner"),
                }
            }

            Some(Json(
                buckets
                    .iter()
                    .map(|(wins, losses)| wins / (wins + losses))
                    .collect(),
            ))
        })
        .await
    } else {
        None
    }
}

pub async fn top_all_inner(conn: &RatingsDbConn) -> Vec<RankingPlayer> {
    conn.run(|c| {
        let mut stmt = c
            .prepare(
                "SELECT 
                    player_ratings.id as id, char_id, 
                    wins, losses, 
                    value, deviation, last_decay, 
                    name, platform, vip_status, cheater_status, hidden_status
                 FROM ranking_global
                 NATURAL JOIN player_ratings
                 NATURAL JOIN players
                 LEFT JOIN vip_status ON vip_status.id = player_ratings.id
                 LEFT JOIN cheater_status ON cheater_status.id = player_ratings.id
                 LEFT JOIN hidden_status ON hidden_status.id = player_ratings.id
                 LIMIT 100",
            )
            .unwrap();
        let mut rows = stmt.query(params![]).unwrap();

        let mut res = Vec::with_capacity(100);
        let mut i = 1;

        while let Some(row) = rows.next().unwrap() {
            let name = row.get("name").unwrap();
            let platform = row.get("platform").unwrap();
            let vip_status = row.get("vip_status").unwrap();
            let cheater_status = row.get("cheater_status").unwrap();
            let hidden_status = row.get("hidden_status").unwrap();
            res.push(RankingPlayer::from_db(
                i,
                name,
                platform,
                vip_status,
                cheater_status,
                hidden_status,
                RatedPlayer::from_row(row),
            ));
            i += 1;
        }

        res
    })
    .await
}

#[get("/api/active_players")]
pub async fn active_players(conn: RatingsDbConn) -> Json<Vec<i64>> {
    Json(
        conn.run(|conn| {
            let now = Utc::now().timestamp();

            let mut res = Vec::new();
            for i in 0..14 {
                let before = now - i * 24 * 60 * 60;
                let after = now - (i + 1) * 24 * 60 * 60;

                res.push(
                    conn.query_row(
                        "SELECT COUNT(id) FROM
                        (SELECT id_a AS id FROM games WHERE timestamp > ? AND timestamp < ?
                        UNION
                        SELECT id_b AS id FROM games WHERE timestamp > ? AND timestamp < ?)",
                        params![after, before, after, before],
                        |r| r.get(0),
                    )
                    .unwrap(),
                );
            }

            res
        })
        .await,
    )
}

#[derive(Serialize)]
pub struct PlayerLookupPlayer {
    id: String,
    name: String,
    characters: Vec<PlayerLookupCharacter>,
}

#[derive(Serialize)]
pub struct PlayerLookupCharacter {
    shortname: &'static str,
    rating: i64,
    deviation: i64,
    game_count: i64,
}

#[get("/api/player_lookup?<name>")]
pub async fn player_lookup(conn: RatingsDbConn, name: String) -> Json<Vec<PlayerLookupPlayer>> {
    Json(
        conn.run(move |conn| {
            let players = {
                let mut stmt = conn
                    .prepare(
                        "SELECT id, name 
                    FROM player_names
                    WHERE name LIKE ?
                    ",
                    )
                    .unwrap();
                let mut rows = stmt.query(params!(name)).unwrap();

                let mut players = Vec::new();
                while let Some(row) = rows.next().unwrap() {
                    players.push((
                        row.get::<_, i64>(0).unwrap(),
                        row.get::<_, String>(1).unwrap(),
                    ));
                }

                players
            };

            let mut r = Vec::new();
            let mut stmt = conn
                .prepare(
                    "SELECT char_id, value, deviation, wins + losses as game_count
                        FROM player_ratings
                        WHERE id = ? ",
                )
                .unwrap();
            for (id, name) in players {
                let mut rows = stmt.query(params![id]).unwrap();

                let mut characters = Vec::new();
                while let Some(row) = rows.next().unwrap() {
                    characters.push(PlayerLookupCharacter {
                        shortname: website::CHAR_NAMES[row.get::<_, usize>(0).unwrap()].0,
                        rating: row.get::<_, f64>(1).unwrap().round() as i64,
                        deviation: (row.get::<_, f64>(2).unwrap() * 2.0).round() as i64,
                        game_count: row.get(3).unwrap(),
                    });
                }

                r.push(PlayerLookupPlayer {
                    id: format!("{:X}", id),
                    name,
                    characters,
                });
            }

            r
        })
        .await,
    )
}

#[derive(Serialize)]
pub struct SearchResultPlayer {
    name: String,
    platform: &'static str,
    vip_status: Option<String>,
    cheater_status: Option<String>,
    hidden_status: Option<String>,
    id: String,
    character: String,
    character_short: String,
    rating_value: i64,
    rating_deviation: i64,
    game_count: i32,
}

#[get("/api/search?<name>")]
pub async fn search(conn: RatingsDbConn, name: String) -> Json<Vec<SearchResultPlayer>> {
    Json(search_inner(&conn, name, false).await)
}

#[get("/api/search_exact?<name>")]
pub async fn search_exact(conn: RatingsDbConn, name: String) -> Json<Vec<SearchResultPlayer>> {
    Json(search_inner(&conn, name, true).await)
}

pub async fn search_inner(
    conn: &RatingsDbConn,
    search: String,
    exact: bool,
) -> Vec<SearchResultPlayer> {
    conn.run(move |c| {
        info!("Searching for {}", search);

        let mut stmt = c
            .prepare(
                "SELECT * FROM
                    player_names
                    NATURAL JOIN players
                    NATURAL JOIN player_ratings
                    LEFT JOIN vip_status ON vip_status.id = player_names.id
                    LEFT JOIN cheater_status ON cheater_status.id = player_names.id
                    LEFT JOIN hidden_status ON hidden_status.id = player_names.id
                    WHERE name LIKE ?
                    ORDER BY wins DESC
                    LIMIT 1000
                    ",
            )
            .unwrap();

        let mut rows = if exact {
            stmt.query(params![search])
        } else {
            stmt.query(params![format!("%{}%", search)])
        }
        .unwrap();

        let mut res = Vec::new();

        while let Some(row) = rows.next().unwrap() {
            let rating: Rating =
                Rating::new(row.get("value").unwrap(), row.get("deviation").unwrap());
            let platform: i64 = row.get("platform").unwrap();
            res.push(SearchResultPlayer {
                name: row.get("name").unwrap(),
                platform: to_platform_string(platform),
                id: format!("{:X}", row.get::<_, i64>("id").unwrap()),
                character: website::CHAR_NAMES[row.get::<_, usize>("char_id").unwrap()]
                    .1
                    .to_owned(),
                character_short: website::CHAR_NAMES[row.get::<_, usize>("char_id").unwrap()]
                    .0
                    .to_owned(),
                rating_value: rating.value.round() as i64,
                rating_deviation: (rating.deviation * 2.0).round() as i64,
                game_count: row.get::<_, i32>("wins").unwrap()
                    + row.get::<_, i32>("losses").unwrap(),
                vip_status: row.get::<_, Option<String>>("vip_status").unwrap(),
                cheater_status: row.get::<_, Option<String>>("cheater_status").unwrap(),
                hidden_status: row.get::<_, Option<String>>("hidden_status").unwrap(),
            });
        }
        res.into_iter().collect()
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
                "SELECT 
                    player_ratings.id as id, char_id, 
                    wins, losses, 
                    value, deviation, last_decay, 
                    name, platform, vip_status, cheater_status, hidden_status
                 FROM ranking_character
                 NATURAL JOIN player_ratings
                 NATURAL JOIN players
                 LEFT JOIN vip_status ON vip_status.id = player_ratings.id
                 LEFT JOIN cheater_status ON cheater_status.id = player_ratings.id
                 LEFT JOIN hidden_status ON hidden_status.id = player_ratings.id
                 WHERE char_id = ?
                 LIMIT 100
                 ",
            )
            .unwrap();
        let mut rows = stmt.query(params![char_id]).unwrap();

        let mut res = Vec::with_capacity(100);
        let mut i = 1;
        while let Some(row) = rows.next().unwrap() {
            let name = row.get("name").unwrap();
            let platform = row.get("platform").unwrap();
            let vip_status = row.get("vip_status").unwrap();
            let cheater_status = row.get("cheater_status").unwrap();
            let hidden_status = row.get("hidden_status").unwrap();
            res.push(RankingPlayer::from_db(
                i,
                name,
                platform,
                vip_status,
                cheater_status,
                hidden_status,
                RatedPlayer::from_row(row),
            ));
            i += 1;
        }

        res
    })
    .await
}

#[derive(Serialize)]
pub struct PlayerData {
    name: String,
    platform: String,
    other_names: Option<Vec<String>>,
    characters: Vec<PlayerCharacterData>,
}

#[derive(Serialize)]
pub struct PlayerDataChar {
    id: String,
    name: String,
    platform: &'static str,
    vip_status: Option<String>,
    cheater_status: Option<String>,
    other_names: Option<Vec<String>>,
    other_characters: Vec<OtherPlayerCharacter>,
    data: PlayerCharacterData,
    pub hidden_status: Option<String>
}

#[derive(Serialize)]
struct OtherPlayerCharacter {
    character_name: String,
    character_shortname: String,
    rating_value: i64,
    rating_deviation: i64,
    game_count: i32,
}

#[derive(Serialize)]
struct PlayerCharacterData {
    character_name: String,
    rating_value: i64,
    rating_deviation: i64,
    global_rank: Option<i32>,
    character_rank: Option<i32>,

    top_rating_value: Option<i64>,
    top_rating_deviation: Option<i64>,
    top_rating_timestamp: Option<String>,

    top_defeated_id: Option<String>,
    top_defeated_char_id: Option<&'static str>,
    top_defeated_name: Option<String>,
    top_defeated_value: Option<i64>,
    top_defeated_deviation: Option<i64>,
    top_defeated_floor: Option<String>,
    top_defeated_timestamp: Option<String>,

    win_rate: f64,
    game_count: i32,
    matchups: Vec<PlayerMatchup>,
}

#[derive(Serialize)]
pub struct PlayerCharacterHistory {
    history: Vec<PlayerSet>,
}

const MATCHUP_MIN_GAMES: i64 = 250;

#[derive(Serialize)]
struct PlayerSet {
    timestamp: String,
    own_rating_value: i64,
    own_rating_deviation: i64,
    floor: String,
    opponent_name: String,
    opponent_platform: &'static str,
    opponent_vip: Option<&'static str>,
    opponent_cheater: Option<&'static str>,
    opponent_hidden: Option<&'static str>,
    opponent_id: String,
    opponent_character: &'static str,
    opponent_character_short: &'static str,
    opponent_rating_value: i64,
    opponent_rating_deviation: i64,
    expected_outcome: String,
    rating_change: String,
    rating_change_class: &'static str,
    rating_change_sequence: String,
    result_wins: i32,
    result_losses: i32,
    result_percent: f64,
}

#[derive(Serialize)]
struct PlayerMatchup {
    character: String,
    game_count: i64,
    win_rate: f64,
    rating_offset: String,
    rating_deviation: i64,
    rating: i64,
    rating_change_class: &'static str,
}

pub async fn get_player_highest_rated_character(conn: &RatingsDbConn, id: i64) -> Option<i64> {
    conn.run(move |conn| {
        conn.query_row(
            "SELECT char_id
        FROM player_ratings
        WHERE id=?
        ORDER BY value - 3.0  * deviation DESC
        LIMIT 1",
            params![id],
            |r| r.get(0),
        )
        .optional()
        .unwrap()
    })
    .await
}

#[derive(Serialize)]
pub struct RecentSet {
    timestamp: String,
    player_a: SetPlayer,
    player_b: SetPlayer,
    win_chance: String,
    wins_a: i64,
    wins_b: i64,
}

#[derive(Serialize)]
pub struct SetPlayer {
    id: i64,
    character_short: &'static str,
    name: String,
    character_name: &'static str,
    rating_value: i64,
    rating_deviation: i64,
}

pub async fn get_recent_sets(conn: &RatingsDbConn) -> Vec<RecentSet> {
    conn.run(move |conn| {
        let _stmt = conn
            .prepare(
                "SELECT
                timestamp,
                id_a, char_a, name_a, platform_a, 
                value_a, deviation_a,
                vip_a.vip_status, cheater_a.cheater_status, hidden_a.hidden_status, 
                id_b, char_b, name_b, platform_b, 
                value_b, deviation_b,
                vip_b.vip_status, cheater_b.cheater_status, hidden_b.hidden_status
            FROM games NATURAL JOIN game_ratings
            LEFT JOIN vip_status AS vip_a on vip_a.id = games.id_a
            LEFT JOIN cheater_status AS cheater_a on cheater_a.id = games.id_a
            LEFT JOIN hidden_status AS hidden_a on hidden_a.id = games.id_a
            LEFT JOIN vip_status AS vip_b on vip_b.id = games.id_b
            LEFT JOIN cheater_status AS cheater_b on cheater_b.id = games.id_b
            LEFT JOIN hidden_status AS hidden_b on hidden_b.id = games.id_b
            ORDER BY timestamp DESC limit 100
                ",
            )
            .unwrap();

        todo!()
    })
    .await
}

pub async fn get_player_char_history(
    conn: &RatingsDbConn,
    id: i64,
    char_id: i64,
    game_count: i64,
    offset: i64,
    group_games: bool,
) -> Option<PlayerCharacterHistory> {
    conn.run(move |conn| {
        let history = {
            let mut stmt = conn
                .prepare_cached(
                    "SELECT
                            timestamp,
                            value_a AS own_value,
                            deviation_a AS own_deviation,
                            game_floor,
                            name_b AS opponent_name,
                            id_b AS opponent_id,
                            char_b AS opponent_character,
                            platform_b AS opponent_platform,
                            value_b AS opponent_value,
                            deviation_b AS opponent_deviation,
                            winner,
                            valid,
                            vip_status,
                            cheater_status,
                            hidden_status
                        FROM games NATURAL JOIN game_ratings
                        LEFT JOIN vip_status ON vip_status.id = games.id_b
                        LEFT JOIN cheater_status ON cheater_status.id = games.id_b
                        LEFT JOIN hidden_status ON hidden_status.id = games.id_b
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
                            platform_a AS opponent_platform,
                            value_a AS opponent_value,
                            deviation_a AS opponent_deviation,
                            winner + 2  as winner,
                            valid,
                            vip_status,
                            cheater_status,
                            hidden_status
                        FROM games NATURAL JOIN game_ratings
                        LEFT JOIN vip_status ON vip_status.id = games.id_a
                        LEFT JOIN cheater_status ON cheater_status.id = games.id_a
                        LEFT JOIN hidden_status ON hidden_status.id = games.id_a
                        WHERE games.id_b = :id AND games.char_b = :char_id

                        ORDER BY timestamp DESC LIMIT :game_count OFFSET :offset",
                )
                .unwrap();

            let mut rows = stmt
                .query(named_params! {
                    ":id" : id,
                    ":char_id": char_id,
                    ":game_count":game_count,
                    ":offset":offset,
                })
                .unwrap();
            let mut history = Vec::<RawPlayerSet>::new();
            while let Some(row) = rows.next().unwrap() {
                let timestamp: i64 = row.get("timestamp").unwrap();
                let own_value: f64 = row.get("own_value").unwrap();
                let own_deviation: f64 = row.get("own_deviation").unwrap();
                let floor: i64 = row.get("game_floor").unwrap();
                let opponent_name: String = row.get("opponent_name").unwrap();
                let opponent_id: i64 = row.get("opponent_id").unwrap();
                let opponent_char: i64 = row.get("opponent_character").unwrap();
                let opponent_value: f64 = row.get("opponent_value").unwrap();
                let opponent_deviation: f64 = row.get("opponent_deviation").unwrap();
                let winner: i64 = row.get("winner").unwrap();
                let valid: bool = row.get("valid").unwrap();
                let opponent_platform: i64 = row.get("opponent_platform").unwrap();
                let opponent_vip: Option<String> = row.get("vip_status").unwrap();
                let opponent_cheater: Option<String> = row.get("cheater_status").unwrap();
                let opponent_hidden: Option<String> = row.get("hidden_status").unwrap();

                if group_games {
                    add_to_grouped_sets(
                        &mut history,
                        timestamp,
                        floor,
                        own_value,
                        own_deviation,
                        opponent_name,
                        opponent_id,
                        opponent_char,
                        to_platform_string(opponent_platform),
                        opponent_value,
                        opponent_deviation,
                        match winner {
                            1 | 4 => true,
                            2 | 3 => false,
                            _ => panic!("Bad winner"),
                        },
                        valid,
                        opponent_vip.is_some(),
                        opponent_cheater.is_some(),
                        opponent_hidden.is_some(),
                    );
                } else {
                    add_ungrouped_set(
                        &mut history,
                        timestamp,
                        floor,
                        own_value,
                        own_deviation,
                        opponent_name,
                        opponent_id,
                        opponent_char,
                        to_platform_string(opponent_platform),
                        opponent_value,
                        opponent_deviation,
                        match winner {
                            1 | 4 => true,
                            2 | 3 => false,
                            _ => panic!("Bad winner"),
                        },
                        valid,
                        opponent_vip.is_some(),
                        opponent_cheater.is_some(),
                        opponent_hidden.is_some(),
                    );
                }
            }

            history
                .into_iter()
                .map(RawPlayerSet::to_formatted_set)
                .collect()
        };

        Some(PlayerCharacterHistory { history })
    })
    .await
}

pub async fn get_player_data_char(
    conn: &RatingsDbConn,
    id: i64,
    char_id: i64,
) -> Option<PlayerDataChar> {
    conn.run(move |conn| {
        if conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM players WHERE id=?)",
                params![id],
                |r| r.get(0),
            )
            .unwrap()
        {
            let (name, platform, vip_status, cheater_status, hidden_status): (
                String,
                i64,
                Option<String>,
                Option<String>,
                Option<String>,
            ) = conn
                .query_row(
                    "SELECT name, platform, vip_status, cheater_status, hidden_status FROM players
                        LEFT JOIN vip_status ON vip_status.id = players.id
                        LEFT JOIN cheater_status ON cheater_status.id = players.id
                        LEFT JOIN hidden_status ON hidden_status.id = players.id
                           WHERE players.id=?
                           ",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                )
                .unwrap();
            info!(
                "Loading data for {} ({})",
                name,
                website::CHAR_NAMES[char_id as usize].0
            );


            let other_names = get_player_other_names(conn, id, &name);

            let other_characters = get_player_other_characters(conn, id);

            let character_data = get_player_character_data(conn, id, char_id).unwrap()?;

            Some(PlayerDataChar {
                id: format!("{:X}", id),
                name,
                platform: to_platform_string(platform),
                vip_status,
                cheater_status,
                other_characters,
                other_names,
                data: character_data,
                hidden_status,
            })
        } else {
            None
        }
    })
    .await
}

fn get_player_other_names(conn: &Connection, id: i64, name: &str) -> Option<Vec<String>> {
    let mut stmt = conn
        .prepare_cached("SELECT name FROM player_names WHERE id=?")
        .unwrap();
    let mut rows = stmt.query(params![id]).unwrap();
    let mut other_names = Vec::new();
    while let Some(row) = rows.next().unwrap() {
        let other_name: String = row.get(0).unwrap();
        if other_name != name && !other_names.contains(&other_name) {
            other_names.push(other_name);
        }
    }

    if other_names.is_empty() {
        None
    } else {
        Some(other_names)
    }
}

fn get_player_other_characters(conn: &Connection, id: i64) -> Vec<OtherPlayerCharacter> {
    let mut stmt = conn
        .prepare_cached(
            "SELECT
            char_id, wins, losses, value, deviation
            FROM player_ratings
            WHERE id=?",
        )
        .unwrap();

    let mut other_characters = Vec::new();

    let mut rows = stmt.query(params![id]).unwrap();

    while let Some(row) = rows.next().unwrap() {
        let char_id: usize = row.get(0).unwrap();
        let game_count: i32 = row.get::<_, i32>(1).unwrap() + row.get::<_, i32>(2).unwrap();
        let rating = Rating::new(row.get(3).unwrap(), row.get(4).unwrap());

        let character_name = website::CHAR_NAMES[char_id].1.to_owned();
        let character_shortname = website::CHAR_NAMES[char_id].0.to_owned();
        other_characters.push(OtherPlayerCharacter {
            character_name,
            character_shortname,
            game_count,
            rating_value: rating.value.round() as i64,
            rating_deviation: (rating.deviation * 2.0).round() as i64,
        });
    }

    other_characters.sort_by_key(|c| c.rating_deviation as i64);

    other_characters
}

fn get_player_character_data(
    conn: &Connection,
    id: i64,
    char_id: i64,
) -> Result<Option<PlayerCharacterData>> {
    let (
        wins,
        losses,
        value,
        deviation,
        top_rating_value,
        top_rating_deviation,
        top_rating_timestamp,
        top_defeated_id,
        top_defeated_char_id,
        top_defeated_name,
        top_defeated_value,
        top_defeated_deviation,
        top_defeated_floor,
        top_defeated_timestamp,
        global_rank,
        character_rank,
    ) = match conn.query_row(
        "SELECT 
            wins, losses, value, deviation, 
            top_rating_value, top_rating_deviation, top_rating_timestamp, 

            top_defeated_id, top_defeated_char_id, top_defeated_name,
            top_defeated_value, top_defeated_deviation, top_defeated_floor,
            top_defeated_timestamp,

            global_rank, character_rank
        FROM player_ratings
            LEFT JOIN ranking_global ON
                ranking_global.id = player_ratings.id AND
                ranking_global.char_id = player_ratings.char_id
            LEFT JOIN ranking_character ON
                ranking_character.id = player_ratings.id AND
                ranking_character.char_id = player_ratings.char_id
            WHERE player_ratings.id=? AND player_ratings.char_id=?",
        params![id, char_id],
        |row| {
            Ok((
                row.get::<_, i32>(0).unwrap(),
                row.get::<_, i32>(1).unwrap(),
                row.get::<_, f64>(2).unwrap(),
                row.get::<_, f64>(3).unwrap(),
                //top rating
                row.get::<_, Option<f64>>(4).unwrap(),
                row.get::<_, Option<f64>>(5).unwrap(),
                row.get::<_, Option<i64>>(6).unwrap(),
                //top defeated
                row.get::<_, Option<i64>>(7).unwrap(),
                row.get::<_, Option<i64>>(8).unwrap(),
                row.get::<_, Option<String>>(9).unwrap(),
                row.get::<_, Option<f64>>(10).unwrap(),
                row.get::<_, Option<f64>>(11).unwrap(),
                row.get::<_, Option<i64>>(12).unwrap(),
                row.get::<_, Option<i64>>(13).unwrap(),
                //rank
                row.get::<_, Option<i32>>(14).unwrap(),
                row.get::<_, Option<i32>>(15).unwrap(),
            ))
        },
    ) {
        Ok(x) => x,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    {
        let character_name = website::CHAR_NAMES[char_id as usize].1.to_owned();

        let matchups = {
            let mut stmt = conn
                .prepare_cached(
                    "SELECT
                        opp_char_id,
                        rating_value,
                        rating_deviation,
                        wins,
                        losses
                    FROM player_matchups
                    WHERE id = ?
                        AND char_id = ?",
                )
                .unwrap();

            let mut rows = stmt.query(params![id, char_id]).unwrap();
            let mut matchups = Vec::<PlayerMatchup>::new();
            while let Some(row) = rows.next().unwrap() {
                let opp_char_id: usize = row.get(0).unwrap();
                let rating_value: f64 = row.get::<_, f64>(1).unwrap();
                let rating_deviation: f64 = row.get(2).unwrap();
                let wins: i64 = row.get(3).unwrap();
                let losses: i64 = row.get(4).unwrap();
                let rating_offset = rating_value - value;
                matchups.push(PlayerMatchup {
                    character: website::CHAR_NAMES[opp_char_id].1.to_owned(),
                    game_count: wins + losses,
                    win_rate: (100.0 * wins as f64 / (wins + losses) as f64).round(),
                    rating_offset: format!("{:+.0} ±{:.0}", rating_offset, 2.0 * rating_deviation),
                    rating_deviation: (rating_deviation * 2.0) as i64,
                    rating: rating_value as i64,
                    rating_change_class: if rating_offset >= 50.0 {
                        "rating-up"
                    } else if rating_offset >= 0.0 {
                        "rating-barely-up"
                    } else if rating_offset >= -50.0 {
                        "rating-barely-down"
                    } else {
                        "rating-down"
                    },
                });
            }

            matchups.sort_by_key(|m| -(m.rating as i64));

            matchups
        };

        Ok(Some(PlayerCharacterData {
            character_name,
            game_count: wins + losses,
            win_rate: (100.0 * wins as f64 / (wins + losses) as f64).round(),
            rating_value: value.round() as i64,
            rating_deviation: (deviation * 2.0).round() as i64,
            top_rating_value: top_rating_value.map(|r| r.round() as i64),
            top_rating_deviation: top_rating_deviation.map(|d| (2.0 * d).round() as i64),
            top_rating_timestamp: top_rating_timestamp.map(|t| {
                NaiveDateTime::from_timestamp_opt(t, 0)
                    .unwrap()
                    .format("%Y-%m-%d")
                    .to_string()
            }),
            top_defeated_id: top_defeated_id.map(|id| format!("{:X}", id)),
            top_defeated_char_id: top_defeated_char_id.map(|id| website::CHAR_NAMES[id as usize].0),
            top_defeated_name,
            top_defeated_value: top_defeated_value.map(|r| r.round() as i64),
            top_defeated_deviation: top_defeated_deviation.map(|r| (2.0 * r).round() as i64),
            top_defeated_floor: top_defeated_floor.map(stringify_floor),
            top_defeated_timestamp: top_defeated_timestamp.map(|t| {
                NaiveDateTime::from_timestamp_opt(t, 0)
                    .unwrap()
                    .format("%Y-%m-%d")
                    .to_string()
            }),
            matchups,
            character_rank,
            global_rank,
        }))
    }
}

struct RawPlayerSet {
    timestamp: i64,
    own_value: f64,
    own_deviation: f64,
    floor: i64,
    opponent_name: String,
    opponent_platform: &'static str,
    opponent_vip: bool,
    opponent_cheater: bool,
    opponent_hidden: bool,
    opponent_id: i64,
    opponent_char: i64,
    opponent_value: f64,
    opponent_deviation: f64,
    valid: bool,

    rating_change_sequence: Vec<f64>,
    result_wins: i32,
    result_losses: i32,
}

impl RawPlayerSet {
    fn to_formatted_set(self) -> PlayerSet {
        let timestamp = NaiveDateTime::from_timestamp_opt(self.timestamp, 0)
            .unwrap()
            .format("%Y-%m-%d %H:%M")
            .to_string();

        let own_rating = Rating::new(self.own_value, self.own_deviation);
        let opp_rating = Rating::new(self.opponent_value, self.opponent_deviation);

        let rsm_deviation =
            (0.5 * self.own_deviation.powf(2.0) + 0.5 * self.opponent_deviation.powf(2.0)).sqrt();

        let expected_outcome = format!(
            "{:.0}%{}",
            own_rating.expected(opp_rating) * 100.0,
            if rsm_deviation < 50.0 {
                ""
            } else if rsm_deviation < 100.0 {
                " ?"
            } else if rsm_deviation < 150.0 {
                " ??"
            } else {
                " ????"
            }
        );

        let rating_change_sum = self.rating_change_sequence.iter().copied().sum::<f64>();
        let average_rating_change =
            rating_change_sum / (self.result_wins + self.result_losses) as f64;

        PlayerSet {
            timestamp,
            own_rating_value: self.own_value.round() as i64,
            own_rating_deviation: (2.0 * self.own_deviation).round() as i64,
            floor: stringify_floor(self.floor),
            opponent_name: self.opponent_name,
            opponent_platform: self.opponent_platform,
            opponent_id: format!("{:X}", self.opponent_id),
            opponent_character_short: website::CHAR_NAMES[self.opponent_char as usize].0,
            opponent_character: website::CHAR_NAMES[self.opponent_char as usize].1,

            opponent_rating_value: self.opponent_value.round() as i64,
            opponent_rating_deviation: (2.0 * self.opponent_deviation).round() as i64,

            rating_change: if self.valid {
                format!("{:+.1}", rating_change_sum,)
            } else {
                format!("---")
            },
            rating_change_class: if !self.valid {
                "rating-same"
            } else if average_rating_change >= 2.0 {
                "rating-up"
            } else if average_rating_change >= 0.0 {
                "rating-barely-up"
            } else if average_rating_change >= -2.0 {
                "rating-barely-down"
            } else {
                "rating-down"
            },
            rating_change_sequence: self.rating_change_sequence.iter().rev().copied().fold(
                String::new(),
                |mut s, c| {
                    s.push_str(&format!("{:+.1} ", c));
                    s
                },
            ),

            result_wins: self.result_wins,
            result_losses: self.result_losses,
            result_percent: (100.0 * self.result_wins as f64
                / (self.result_wins + self.result_losses) as f64)
                .round(),

            opponent_cheater: self.opponent_cheater.then_some("Cheater"),
            opponent_vip: self.opponent_vip.then_some("VIP"),
            opponent_hidden: self.opponent_hidden.then_some("Hidden"),

            expected_outcome,
        }
    }
}

fn stringify_floor(floor: i64) -> String {
    match floor {
        f @ 1..=10 => format!("F{:0}", f),
        _ => "C".to_owned(),
    }
}

fn add_to_grouped_sets(
    sets: &mut Vec<RawPlayerSet>,
    timestamp: i64,
    floor: i64,
    own_value: f64,
    own_deviation: f64,
    opponent_name: String,
    opponent_id: i64,
    opponent_char: i64,
    opponent_platform: &'static str,
    opponent_value: f64,
    opponent_deviation: f64,
    winner: bool,
    valid: bool,
    opponent_vip: bool,
    opponent_cheater: bool,
    opponent_hidden: bool,
) {
    let own_rating = Rating::new(own_value, own_deviation);
    let opp_rating = Rating::new(opponent_value, opponent_deviation);

    let rating_change = if valid {
        Rating::rating_change(own_rating, opp_rating, if winner { 1.0 } else { 0.0 })
    } else {
        0.0
    };

    if let Some(set) = sets.last_mut().filter(|set| {
        set.opponent_id == opponent_id && set.opponent_char == opponent_char && set.valid == valid
    }) {
        set.timestamp = timestamp;
        set.own_value = own_value;
        set.own_deviation = own_deviation;
        set.opponent_value = opponent_value;
        set.opponent_deviation = opponent_deviation;

        set.rating_change_sequence.push(rating_change);
        match winner {
            true => set.result_wins += 1,
            false => set.result_losses += 1,
        }
    } else {
        sets.push(RawPlayerSet {
            timestamp,
            own_value,
            own_deviation,
            floor,
            opponent_name,
            opponent_platform,
            opponent_vip,
            opponent_cheater,
            opponent_hidden,
            opponent_id,
            opponent_char,
            opponent_value,
            opponent_deviation,
            valid,
            rating_change_sequence: vec![rating_change],
            result_wins: if winner { 1 } else { 0 },
            result_losses: if winner { 0 } else { 1 },
        });
    }
}

fn add_ungrouped_set(
    sets: &mut Vec<RawPlayerSet>,
    timestamp: i64,
    floor: i64,
    own_value: f64,
    own_deviation: f64,
    opponent_name: String,
    opponent_id: i64,
    opponent_char: i64,
    opponent_platform: &'static str,
    opponent_value: f64,
    opponent_deviation: f64,
    winner: bool,
    valid: bool,
    opponent_vip: bool,
    opponent_cheater: bool,
    opponent_hidden: bool,
) {
    let own_rating = Rating::new(own_value, own_deviation);
    let opp_rating = Rating::new(opponent_value, opponent_deviation);

    let rating_change = if valid {
        Rating::rating_change(own_rating, opp_rating, if winner { 1.0 } else { 0.0 })
    } else {
        0.0
    };

    sets.push(RawPlayerSet {
        timestamp,
        own_value,
        own_deviation,
        floor,
        opponent_name,
        opponent_platform,
        opponent_vip,
        opponent_cheater,
        opponent_hidden,
        opponent_id,
        opponent_char,
        opponent_value,
        opponent_deviation,
        valid,
        rating_change_sequence: vec![rating_change],
        result_wins: if winner { 1 } else { 0 },
        result_losses: if winner { 0 } else { 1 },
    });
}

#[derive(Serialize)]
pub struct CharacterMatchups {
    name: String,
    matchups: Vec<Matchup>,
}

#[derive(Serialize)]
pub struct Matchup {
    matchup: String,
    win_rate: f64,
    game_count: i64,
    rating_delta: String,
    expected: f64,
    suspicious: bool,
    evaluation: &'static str,
}

fn get_evaluation(r: f64, game_count: i64) -> &'static str {
    if game_count < MATCHUP_MIN_GAMES {
        return "none";
    }

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

pub async fn get_matchups(conn: &RatingsDbConn, table: &'static str) -> Vec<CharacterMatchups> {
    conn.run(move |conn| {
        let tx = conn.transaction().unwrap();
        let mut all_matchups = FxHashMap::default();

        let mut stmt = tx
            .prepare(&format!(
                "SELECT char_id, opp_char_id, rating_value, rating_deviation, wins, losses FROM {}",
                table
            ))
            .unwrap();

        let mut rows = stmt.query([]).unwrap();

        while let Some(row) = rows.next().unwrap() {
            let char_id: i64 = row.get(0).unwrap();
            let opp_char_id: i64 = row.get(1).unwrap();
            let rating_value: f64 = row.get(2).unwrap();
            let rating_deviation: f64 = row.get(3).unwrap();
            let wins: i64 = row.get(4).unwrap();
            let losses: i64 = row.get(5).unwrap();

            all_matchups.insert(
                (char_id, opp_char_id),
                (rating_value, rating_deviation, wins, losses),
            );
        }

        (0..website::CHAR_NAMES.len() as i64)
            .map(|c| CharacterMatchups {
                name: website::CHAR_NAMES[c as usize].1.to_owned(),
                matchups: (0..website::CHAR_NAMES.len() as i64)
                    .map(|o| {
                        let (own_value, own_deviation, wins, losses) =
                            *all_matchups.get(&(c, o)).unwrap_or(&(1500.0, 350.0, 0, 0));

                        let (opp_value, opp_deviation, ..) =
                            *all_matchups.get(&(o, c)).unwrap_or(&(1500.0, 350.0, 0, 0));

                        let expected = Rating::new(own_value, own_deviation)
                            .expected(Rating::new(opp_value, opp_deviation));

                        Matchup {
                            matchup: format!(
                                "{} vs {}",
                                website::CHAR_NAMES[c as usize].0,
                                website::CHAR_NAMES[o as usize].0
                            ),
                            win_rate: (100.0 * wins as f64 / (wins + losses) as f64).round(),
                            game_count: wins + losses,
                            rating_delta: format!("{:+.0}", own_value - opp_value),
                            expected: (100.0 * expected).round(),
                            evaluation: get_evaluation(expected, wins + losses),
                            suspicious: wins + losses < MATCHUP_MIN_GAMES,
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
            .query_row(
                "SELECT SUM(player_count) FROM player_floor_distribution",
                [],
                |r| r.get(0),
            )
            .unwrap();

        let total_games: i64 = conn
            .query_row(
                "SELECT SUM(game_count) FROM player_floor_distribution",
                [],
                |r| r.get(0),
            )
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
        SELECT player_count_cum 
        FROM player_rating_distribution
        ORDER BY player_count_cum DESC",
                [],
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

#[derive(Serialize)]
pub struct RankCharacterPopularities {
    rating_min: usize,
    rating_max: usize,
    characters: Vec<RankCharacterPopularity>,
}

#[derive(Serialize)]
struct RankCharacterPopularity {
    popularity: f64,
    delta: f64,
    evaluation: &'static str,
}

pub async fn character_popularity(
    conn: &RatingsDbConn,
) -> (Vec<f64>, Vec<RankCharacterPopularities>) {
    conn.run(move |conn| {
        let global_popularities = {
            let mut stmt = conn
                .prepare(
                    "SELECT
                        popularity
                        FROM character_popularity_global
                        ORDER BY char_id ASC",
                )
                .unwrap();

            let mut rows = stmt.query([]).unwrap();
            let mut v = Vec::with_capacity(website::CHAR_NAMES.len());

            while let Some(row) = rows.next().unwrap() {
                let popularity: f64 = row.get(0).unwrap();
                v.push((popularity * 1000.0).round() / 10.0);
            }

            v
        };

        let rank_popularites = {
            let mut rank_popularities = Vec::with_capacity(rater::POP_RATING_BRACKETS);

            for r in 0..rater::POP_RATING_BRACKETS {
                let mut stmt = conn
                    .prepare(
                        "SELECT
                        char_id, popularity
                        FROM character_popularity_rating
                        WHERE rating_bracket = ?
                        ORDER BY char_id ASC",
                    )
                    .unwrap();

                let mut rows = stmt.query(params![r]).unwrap();

                let mut res = RankCharacterPopularities {
                    rating_min: if r > 0 { 1000 + r * 100 } else { 0 },
                    rating_max: if r < rater::POP_RATING_BRACKETS - 1 {
                        1000 + (r + 1) * 100
                    } else {
                        3000
                    },
                    characters: Vec::with_capacity(website::CHAR_NAMES.len()),
                };

                while let Some(row) = rows.next().unwrap() {
                    let char_id: usize = row.get(0).unwrap();
                    let popularity: f64 = row.get(1).unwrap();
                    let popularity = (popularity * 1000.0).round() / 10.0;
                    let delta =
                        (popularity - global_popularities[char_id]) / global_popularities[char_id];
                    let delta = (delta * 1000.0).round() / 10.0;

                    res.characters.push(RankCharacterPopularity {
                        popularity,
                        delta,
                        evaluation: if delta > 50.0 {
                            "verygood"
                        } else if delta > 15.0 {
                            "good"
                        } else if delta > -15.0 {
                            "ok"
                        } else if delta > -50.0 {
                            "bad"
                        } else {
                            "verybad"
                        },
                    });
                }

                rank_popularities.push(res);
            }

            rank_popularities
        };

        (global_popularities, rank_popularites)
    })
    .await
}

#[derive(Serialize)]
pub struct FraudStats {
    character_name: &'static str,
    player_count: i64,
    average_offset: String,
}

pub async fn get_fraud(conn: &RatingsDbConn) -> Vec<FraudStats> {
    conn.run(move |conn| {
        let mut stmt = conn
            .prepare(
                "SELECT char_id, player_count, avg_delta FROM fraud_index ORDER BY avg_delta DESC",
            )
            .unwrap();

        let mut rows = stmt.query(params![]).unwrap();

        let mut res = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            res.push(FraudStats {
                character_name: website::CHAR_NAMES[row.get::<_, usize>(0).unwrap()].1,
                player_count: row.get(1).unwrap(),
                average_offset: format!("{:+.1}", (row.get::<_, f64>(2).unwrap())),
            });
        }

        res
    })
    .await
}

pub async fn get_fraud_higher_rated(conn: &RatingsDbConn) -> Vec<FraudStats> {
    conn.run(move |conn| {
        let mut stmt = conn
            .prepare("SELECT char_id, player_count, avg_delta FROM fraud_index_higher_rated ORDER BY avg_delta DESC")
            .unwrap();

        let mut rows = stmt.query(params![]).unwrap();

        let mut res = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            res.push(FraudStats {
                character_name: website::CHAR_NAMES[row.get::<_, usize>(0).unwrap()].1,
                player_count: row.get(1).unwrap(),
                average_offset: format!("{:+.1}", (row.get::<_, f64>(2).unwrap())),
            });
        }

        res
    })
    .await
}

pub async fn get_fraud_highest_rated(conn: &RatingsDbConn) -> Vec<FraudStats> {
    conn.run(move |conn| {
        let mut stmt = conn
            .prepare("SELECT char_id, player_count, avg_delta FROM fraud_index_highest_rated ORDER BY avg_delta DESC")
            .unwrap();

        let mut rows = stmt.query(params![]).unwrap();

        let mut res = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            res.push(FraudStats {
                character_name: website::CHAR_NAMES[row.get::<_, usize>(0).unwrap()].1,
                player_count: row.get(1).unwrap(),
                average_offset: format!("{:+.1}", (row.get::<_, f64>(2).unwrap())),
            });
        }

        res
    })
    .await
}

#[derive(Serialize)]
pub struct VipPlayer {
    id: String,
    name: String,
    vip_status: Option<String>,
}

pub async fn get_supporters(conn: &RatingsDbConn) -> Vec<VipPlayer> {
    conn.run(move |conn| {
        let mut stmt = conn
            .prepare(
                "SELECT id, name, vip_status
                    FROM vip_status NATURAL JOIN players",
            )
            .unwrap();

        let mut rows = stmt.query(params![]).unwrap();

        let mut res = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            res.push(VipPlayer {
                id: format!("{:X}", row.get::<_, i64>(0).unwrap()),
                name: row.get(1).unwrap(),
                vip_status: row.get(2).unwrap(),
            });
        }

        res.reverse();

        res
    })
    .await
}

#[derive(Serialize)]
pub struct RatingDiffStats {
    below_400: f64,
    below_300: f64,
    below_200: f64,
    below_100: f64,
    over_100: f64,
    over_200: f64,
    over_300: f64,
    over_400: f64,
    difference_amounts: Vec<i64>,
    difference_counts: Vec<f64>,
}

#[get("/api/player_rating_experience/<player_id>")]
pub async fn rating_experience_player(
    conn: RatingsDbConn,
    player_id: &str,
) -> Json<RatingDiffStats> {
    let id = i64::from_str_radix(player_id, 16).unwrap();

    Json(
        conn.run(move |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT id_a, id_b, value_a, value_b
                    FROM game_ratings
                    WHERE deviation_a < ?
                        AND deviation_b < ?
                        AND (id_a = ?  OR id_b = ?)",
                )
                .unwrap();

            let mut rows = stmt
                .query(params![rater::LOW_DEVIATION, rater::LOW_DEVIATION, id, id,])
                .unwrap();

            let mut counts: FxHashMap<i64, i64> = Default::default();

            let mut total = 0.0;
            let mut over_100 = 0.0;
            let mut over_200 = 0.0;
            let mut over_300 = 0.0;
            let mut over_400 = 0.0;
            let mut below_100 = 0.0;
            let mut below_200 = 0.0;
            let mut below_300 = 0.0;
            let mut below_400 = 0.0;

            while let Some(row) = rows.next().unwrap() {
                let id_a: i64 = row.get(0).unwrap();
                let id_b: i64 = row.get(1).unwrap();
                let a: f64 = row.get(2).unwrap();
                let b: f64 = row.get(3).unwrap();

                if id_a == id {
                    let delta = b - a;

                    if delta > 100.0 {
                        over_100 += 1.0;
                    }
                    if delta > 200.0 {
                        over_200 += 1.0;
                    }
                    if delta > 300.0 {
                        over_300 += 1.0;
                    }
                    if delta > 400.0 {
                        over_400 += 1.0;
                    }
                    if delta < -100.0 {
                        below_100 += 1.0
                    }
                    if delta < -200.0 {
                        below_200 += 1.0
                    }
                    if delta < -300.0 {
                        below_300 += 1.0
                    }
                    if delta < -400.0 {
                        below_400 += 1.0
                    }
                    total += 1.0;

                    let bucket = ((delta + 12.5) / 25.0).floor() as i64;

                    *counts.entry(bucket).or_default() += 1;
                }

                if id_b == id {
                    let delta = a - b;

                    if delta > 100.0 {
                        over_100 += 1.0;
                    }
                    if delta > 200.0 {
                        over_200 += 1.0;
                    }
                    if delta > 300.0 {
                        over_300 += 1.0;
                    }
                    if delta > 400.0 {
                        over_400 += 1.0;
                    }
                    if delta < -100.0 {
                        below_100 += 1.0
                    }
                    if delta < -200.0 {
                        below_200 += 1.0
                    }
                    if delta < -300.0 {
                        below_300 += 1.0
                    }
                    if delta < -400.0 {
                        below_400 += 1.0
                    }
                    total += 1.0;

                    let bucket = ((delta + 12.5) / 25.0).floor() as i64;

                    *counts.entry(bucket).or_default() += 1;
                }
            }

            let min_bucket = -30;
            let max_bucket = 30;
            //let min_bucket = *counts.keys().min().unwrap();
            //let max_bucket = *counts.keys().max().unwrap();

            RatingDiffStats {
                over_100: over_100 / total,
                over_200: over_200 / total,
                over_300: over_300 / total,
                over_400: over_400 / total,
                below_100: below_100 / total,
                below_200: below_200 / total,
                below_300: below_300 / total,
                below_400: below_400 / total,
                difference_amounts: (min_bucket..=max_bucket)
                    .into_iter()
                    .map(|r| r * 25.0 as i64)
                    .collect(),
                difference_counts: (min_bucket..=max_bucket)
                    .into_iter()
                    .map(|r| (counts.get(&r).copied().unwrap_or(0) as f64 / total * 100.0))
                    .collect(),
            }
        })
        .await,
    )
}

#[get("/api/rating_experience?<min_rating>&<max_rating>")]
pub async fn rating_experience(
    conn: RatingsDbConn,
    min_rating: i64,
    max_rating: i64,
) -> Json<RatingDiffStats> {
    Json(
        conn.run(move |conn| {
            let min_rating_glicko2 = (min_rating as f64 - 1500.0) / 173.718;
            let max_rating_glicko2 = (max_rating as f64 - 1500.0) / 173.718;
            let mut stmt = conn
                .prepare(
                    "SELECT value_a, value_b
                    FROM game_ratings
                    WHERE deviation_a < ? AND deviation_b < ? AND
                        ((value_a > ? AND value_a < ?)
                        OR
                        (value_b > ? AND value_b < ?))",
                )
                .unwrap();

            let mut rows = stmt
                .query(params![
                    rater::LOW_DEVIATION,
                    rater::LOW_DEVIATION,
                    min_rating_glicko2,
                    max_rating_glicko2,
                    min_rating_glicko2,
                    max_rating_glicko2,
                ])
                .unwrap();

            let mut counts: FxHashMap<i64, i64> = Default::default();

            let mut total = 0.0;
            let mut over_100 = 0.0;
            let mut over_200 = 0.0;
            let mut over_300 = 0.0;
            let mut over_400 = 0.0;
            let mut below_100 = 0.0;
            let mut below_200 = 0.0;
            let mut below_300 = 0.0;
            let mut below_400 = 0.0;

            while let Some(row) = rows.next().unwrap() {
                let a: f64 = row.get(0).unwrap();
                let b: f64 = row.get(1).unwrap();
                let a = a * 173.718 + 1500.0;
                let b = b * 173.718 + 1500.0;

                if a > min_rating as f64 && a < max_rating as f64 {
                    let delta = b - a;

                    if delta > 100.0 {
                        over_100 += 1.0;
                    }
                    if delta > 200.0 {
                        over_200 += 1.0;
                    }
                    if delta > 300.0 {
                        over_300 += 1.0;
                    }
                    if delta > 400.0 {
                        over_400 += 1.0;
                    }
                    if delta < -100.0 {
                        below_100 += 1.0
                    }
                    if delta < -200.0 {
                        below_200 += 1.0
                    }
                    if delta < -300.0 {
                        below_300 += 1.0
                    }
                    if delta < -400.0 {
                        below_400 += 1.0
                    }
                    total += 1.0;

                    let bucket = ((delta + 12.5) / 25.0).floor() as i64;

                    *counts.entry(bucket).or_default() += 1;
                }

                if b > min_rating as f64 && b < max_rating as f64 {
                    let delta = a - b;

                    if delta > 100.0 {
                        over_100 += 1.0;
                    }
                    if delta > 200.0 {
                        over_200 += 1.0;
                    }
                    if delta > 300.0 {
                        over_300 += 1.0;
                    }
                    if delta > 400.0 {
                        over_400 += 1.0;
                    }
                    if delta < -100.0 {
                        below_100 += 1.0
                    }
                    if delta < -200.0 {
                        below_200 += 1.0
                    }
                    if delta < -300.0 {
                        below_300 += 1.0
                    }
                    if delta < -400.0 {
                        below_400 += 1.0
                    }
                    total += 1.0;

                    let bucket = ((delta + 12.5) / 25.0).floor() as i64;

                    *counts.entry(bucket).or_default() += 1;
                }
            }

            let min_bucket = -30;
            let max_bucket = 30;
            //let min_bucket = *counts.keys().min().unwrap();
            //let max_bucket = *counts.keys().max().unwrap();

            RatingDiffStats {
                over_100: over_100 / total,
                over_200: over_200 / total,
                over_300: over_300 / total,
                over_400: over_400 / total,
                below_100: below_100 / total,
                below_200: below_200 / total,
                below_300: below_300 / total,
                below_400: below_400 / total,
                difference_amounts: (min_bucket..=max_bucket)
                    .into_iter()
                    .map(|r| r * 25.0 as i64)
                    .collect(),
                difference_counts: (min_bucket..=max_bucket)
                    .into_iter()
                    .map(|r| (counts.get(&r).copied().unwrap_or(0) as f64 / total * 100.0))
                    .collect(),
            }
        })
        .await,
    )
}

#[derive(Serialize)]
pub struct FloorRatingDistributions {
    ratings: Vec<i64>,
    floors: FxHashMap<i64, Vec<f64>>,
    overall: Vec<f64>,
}

#[get("/api/floor_rating_distribution")]
pub async fn floor_rating_distribution(conn: RatingsDbConn) -> Json<FloorRatingDistributions> {
    Json(
        conn.run(move |conn| {
            let mut stmt = conn
                .prepare(
                    "SELECT floor, value
                    FROM players NATURAL JOIN player_ratings
                    WHERE deviation < ?",
                )
                .unwrap();

            let mut rows = stmt.query(params![rater::LOW_DEVIATION]).unwrap();

            let mut totals: FxHashMap<i64, FxHashMap<i64, i64>> = Default::default();
            let mut overall: FxHashMap<i64, i64> = Default::default();

            while let Some(row) = rows.next().unwrap() {
                let floor: i64 = row.get(0).unwrap();
                let value: f64 = row.get(1).unwrap();
                //let value = value * 173.718 + 1500.0;

                let bucket = ((value + 25.0) / 50.0).floor() as i64;

                *totals.entry(floor).or_default().entry(bucket).or_default() += 1;
                *overall.entry(bucket).or_default() += 1;
            }

            let min_bucket = *totals.values().flat_map(|f| f.keys()).min().unwrap();
            let max_bucket = *totals.values().flat_map(|f| f.keys()).max().unwrap();

            FloorRatingDistributions {
                ratings: (min_bucket..max_bucket)
                    .into_iter()
                    .map(|r| r * 50)
                    .collect(),
                floors: totals
                    .into_iter()
                    .map(|(f, sums)| {
                        //let max: i64 = *sums.values().max().unwrap();
                        (
                            f,
                            (min_bucket..max_bucket)
                                .into_iter()
                                .map(|r| (sums.get(&r).copied().unwrap_or(0) as f64))
                                .collect(),
                        )
                    })
                    .collect(),
                overall: (min_bucket..max_bucket)
                    .into_iter()
                    .map(|r| (overall.get(&r).copied().unwrap_or(0) as f64))
                    .collect(),
            }
        })
        .await,
    )
}

#[get("/api/outcomes")]
pub async fn outcomes(conn: RatingsDbConn) -> Json<(Vec<i64>, Vec<f64>, Vec<f64>)> {
    Json(
        conn.run(move |conn| {
            let mut outcomes = vec![(0.0, 0.0); 101];

            let mut stmt = conn
                .prepare(
                    "SELECT
                value_a, deviation_a, value_b, deviation_b, winner
                FROM games NATURAL JOIN game_ratings",
                )
                .unwrap();

            let mut rows = stmt.query(params![]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                let rating_a = Rating::new(row.get(0).unwrap(), row.get(1).unwrap());
                let rating_b = Rating::new(row.get(2).unwrap(), row.get(3).unwrap());
                let winner: i64 = row.get(4).unwrap();

                let p = Rating::expected(rating_a, rating_b);

                let o = outcomes.get_mut((p * 100.0).round() as usize).unwrap();
                if winner == 1 {
                    o.0 += 1.0;
                }
                o.1 += 1.0;
            }

            (
                (0..=100).into_iter().collect(),
                outcomes
                    .into_iter()
                    .map(|(wins, total)| wins / total)
                    .collect(),
                (0..=100).into_iter().map(|i| i as f64 / 100.0).collect(),
            )
        })
        .await,
    )
}

#[get("/api/hide/<player>")]
pub async fn start_hide_player(conn: RatingsDbConn, player: &str) -> Json<String> {
    let id = i64::from_str_radix(&player, 16).unwrap();
    let player_code = Alphanumeric.sample_string(&mut rand::thread_rng(), 8);

    let code = conn.run(move |conn| {
        let tx = conn.transaction().unwrap();
        
        let exists: i64 = tx.query_row(
            "SELECT count(id) FROM hidden_status WHERE id=? and hidden_status is not null",
            params![&id],
            |r| r.get(0)
        ).unwrap();

        if exists > 0 {
            tx.execute(
                "UPDATE hidden_status SET code=? WHERE id=?",
                params![&player_code, &id],
            ).unwrap();

        } else {
            tx.execute(
                "INSERT or replace INTO hidden_status(id, hidden_status, code, notes)
                VALUES(?, NULL, ?, 'PlayerAutomated')",
                params![&id, &player_code]
            ).unwrap();
        }
        let _ = tx.commit();
        player_code
    }).await;

    Json(code)
}

#[get("/api/hide/poll/<player>")]
pub async fn poll_hide_player(conn: RatingsDbConn, player: &str) -> Json<bool> {
    let id = i64::from_str_radix(&player, 16).unwrap();

    let code: String = conn.run(move |conn| {
        conn.query_row(
            "SELECT code FROM hidden_status WHERE id=?",
            params![&id],
            |r| r.get(0),
        ).unwrap()
    }).await;

    if code.is_empty() {
        Json(false)
    } else {
        let json = ggst_api::get_player_stats(id.to_string()).await;
        let lookup = format!("PublicComment\":\"{code}");

        let found = match json {
            Ok(json) => json.contains(&lookup),
            Err(er) => {
                println!("error {}", er);
                false
            }
        };

        if found {
            let _ = conn.run(move |conn| {
                let exists: i64 = conn.query_row(
                    "SELECT count(id) FROM hidden_status WHERE id=? and hidden_status is not null",
                    params![&id],
                    |r| r.get(0)
                ).unwrap();
        
                if exists > 0 {
                    conn.execute(
                        "UPDATE hidden_status SET hidden_status=NULL, code=NULL WHERE id=?",
                        params![&id]
                    ).unwrap();
                } else {
                    conn.execute(
                        "UPDATE hidden_status SET hidden_status='enabled', code=NULL WHERE id=?",
                        params![&id]
                    ).unwrap();
                }
            }).await;

            return Json(true);
        }
        Json(false)
    }
}

#[get("/api/outcomes_delta")]
pub async fn outcomes_delta(conn: RatingsDbConn) -> Json<(Vec<i64>, Vec<f64>, Vec<f64>)> {
    Json(
        conn.run(move |conn| {
            let mut outcomes = vec![(0.0, 0.0); 201];

            let mut stmt = conn
                .prepare(
                    "SELECT
                value_a, deviation_a, value_b, deviation_b, winner
                FROM games NATURAL JOIN game_ratings",
                )
                .unwrap();

            let mut rows = stmt.query(params![]).unwrap();
            while let Some(row) = rows.next().unwrap() {
                let rating_a = Rating::new(row.get(0).unwrap(), row.get(1).unwrap());
                let rating_b = Rating::new(row.get(2).unwrap(), row.get(3).unwrap());
                let winner: i64 = row.get(4).unwrap();

                let delta = glicko::g(rating_a.deviation)
                    * glicko::g(rating_b.deviation)
                    * (rating_a.value - rating_b.value);
                let delta_category = (delta / 10.0).round() as i32 + 100;
                if delta_category >= 0 && delta_category <= 200 {
                    let delta_category = delta_category as usize;
                    let o = outcomes.get_mut(delta_category).unwrap();
                    if winner == 1 {
                        o.0 += 1.0;
                    }
                    o.1 += 1.0;
                }
            }

            (
                (0..=200).into_iter().map(|i| (i - 100) * 10).collect(),
                (0..=200)
                    .into_iter()
                    .map(|i| (i - 100) * 10)
                    .map(|i| crate::glicko::e(i as f64, 0.0, 0.0))
                    .collect(),
                outcomes
                    .into_iter()
                    .map(|(wins, total)| wins / total)
                    .collect(),
            )
        })
        .await,
    )
}
