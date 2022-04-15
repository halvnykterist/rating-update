use chrono::{Duration, NaiveDateTime, Utc};
use fxhash::FxHashMap;
use glicko2::{Glicko2Rating, GlickoRating};
use rocket::serde::{json::Json, Serialize};
use rusqlite::{named_params, params, Connection, OptionalExtension};

use crate::{
    rater::{self, RatedPlayer},
    website::{self, RatingsDbConn},
};

type Result<T> = std::result::Result<T, anyhow::Error>;

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
    character: String,
    character_short: String,
    name: String,
    game_count: i32,
    rating_value: f64,
    rating_deviation: f64,
    vip_status: Option<String>,
    cheater_status: Option<String>,
}

impl RankingPlayer {
    fn from_db(
        pos: i32,
        name: String,
        vip_status: Option<String>,
        cheater_status: Option<String>,
        rated_player: RatedPlayer,
    ) -> Self {
        Self {
            pos,
            name,
            id: format!("{:X}", rated_player.id),
            character: website::CHAR_NAMES[rated_player.char_id as usize]
                .1
                .to_owned(),
            character_short: website::CHAR_NAMES[rated_player.char_id as usize]
                .0
                .to_owned(),
            game_count: (rated_player.win_count + rated_player.loss_count) as i32,
            rating_value: GlickoRating::from(rated_player.rating).value.round(),
            rating_deviation: (GlickoRating::from(rated_player.rating).deviation * 2.0).round(),
            vip_status,
            cheater_status,
        }
    }
}
#[get("/api/top/all")]
pub async fn top_all(conn: RatingsDbConn) -> Json<Vec<RankingPlayer>> {
    Json(top_all_inner(&conn).await)
}

#[derive(Serialize)]
pub struct Rating {
    value: f64,
    deviation: f64,
    volatility: f64,
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
            if let Some((value, deviation, volatility)) = conn
                .query_row(
                    "SELECT value, deviation, volatility
                                FROM player_ratings
                                WHERE id=? AND char_id=?",
                    params![id, char_id],
                    |r| {
                        Ok((
                            r.get::<_, f64>(0)?,
                            r.get::<_, f64>(1)?,
                            r.get::<_, f64>(2)?,
                        ))
                    },
                )
                .optional()
                .unwrap()
            {
                Some(Json(Rating {
                    value,
                    deviation,
                    volatility,
                }))
            } else {
                None
            }
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
                "SELECT player_ratings.id as id, char_id, wins, losses, value, deviation, volatility, name, vip_status, cheater_status
                 FROM ranking_global
                 NATURAL JOIN player_ratings
                 NATURAL JOIN players
                 LEFT JOIN vip_status ON vip_status.id = player_ratings.id
                 LEFT JOIN cheater_status ON cheater_status.id = player_ratings.id
                 LIMIT 100",
            )
            .unwrap();
        let mut rows = stmt.query(params![]).unwrap();

        let mut res = Vec::with_capacity(100);
        let mut i = 1;

        while let Some(row) = rows.next().unwrap() {
            let name = row.get("name").unwrap();
            let vip_status = row.get("vip_status").unwrap();
            let cheater_status = row.get("cheater_status").unwrap();
            res.push(RankingPlayer::from_db(
                i,
                name,
                vip_status,
                cheater_status,
                RatedPlayer::from_row(row),
            ));
            i += 1;
        }

        res
    })
    .await
}

#[derive(Serialize)]
pub struct SearchResultPlayer {
    name: String,
    vip_status: Option<String>,
    cheater_status: Option<String>,
    id: String,
    character: String,
    character_short: String,
    rating_value: f64,
    rating_deviation: f64,
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
                    NATURAL JOIN player_ratings
                    LEFT JOIN vip_status ON vip_status.id = player_names.id
                    LEFT JOIN cheater_status ON cheater_status.id = player_names.id
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
                character_short: website::CHAR_NAMES[row.get::<_, usize>("char_id").unwrap()]
                    .0
                    .to_owned(),
                rating_value: rating.value.round(),
                rating_deviation: (rating.deviation * 2.0).round(),
                game_count: row.get::<_, i32>("wins").unwrap()
                    + row.get::<_, i32>("losses").unwrap(),
                vip_status: row.get::<_, Option<String>>("vip_status").unwrap(),
                cheater_status: row.get::<_, Option<String>>("cheater_status").unwrap(),
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
                "SELECT player_ratings.id as id, char_id, wins, losses, value, deviation, volatility, name, vip_status, cheater_status
                 FROM ranking_character
                 NATURAL JOIN player_ratings
                 NATURAL JOIN players
                 LEFT JOIN vip_status ON vip_status.id = player_ratings.id
                 LEFT JOIN cheater_status ON cheater_status.id = player_ratings.id
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
            let vip_status = row.get("vip_status").unwrap();
            let cheater_status = row.get("cheater_status").unwrap();
            res.push(RankingPlayer::from_db(
                i,
                name,
                vip_status,
                cheater_status,
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
    other_names: Option<Vec<String>>,
    characters: Vec<PlayerCharacterData>,
}

#[derive(Serialize)]
pub struct PlayerDataChar {
    id: String,
    name: String,
    vip_status: Option<String>,
    cheater_status: Option<String>,
    other_names: Option<Vec<String>>,
    other_characters: Vec<OtherPlayerCharacter>,
    data: PlayerCharacterData,
}

#[derive(Serialize)]
struct OtherPlayerCharacter {
    character_name: String,
    character_shortname: String,
    rating_value: f64,
    rating_deviation: f64,
    game_count: i32,
}

#[derive(Serialize)]
struct PlayerCharacterData {
    character_name: String,
    rating_value: f64,
    rating_deviation: f64,
    global_rank: Option<i32>,
    character_rank: Option<i32>,
    win_rate: f64,
    game_count: i32,
    matchups: Vec<PlayerMatchup>,
}

#[derive(Serialize)]
pub struct PlayerCharacterHistory {
    history: Vec<PlayerSet>,
    recent_games: Vec<PlayerSet>,
}

const MATCHUP_MIN_GAMES: f64 = 250.0;

#[derive(Serialize)]
struct PlayerSet {
    timestamp: String,
    own_rating_value: f64,
    own_rating_deviation: f64,
    floor: String,
    opponent_name: String,
    opponent_vip: Option<String>,
    opponent_cheater: Option<String>,
    opponent_id: String,
    opponent_character: String,
    opponent_character_short: String,
    opponent_rating_value: f64,
    opponent_rating_deviation: f64,
    expected_outcome: f64,
    expected_outcome_evaluation: &'static str,
    expected_outcome_min: f64,
    expected_outcome_max: f64,
    result_wins: i32,
    result_losses: i32,
    result_percent: f64,
}

fn get_expected_outcomes(
    own_value: f64,
    own_deviation: f64,
    opp_value: f64,
    opp_deviation: f64,
) -> (f64, f64, f64, &'static str) {
    let own_min = (own_value - own_deviation).exp();
    let own_avg = (own_value).exp();
    let own_max = (own_value + own_deviation).exp();

    let opp_min = (opp_value - opp_deviation).exp();
    let opp_avg = (opp_value).exp();
    let opp_max = (opp_value + opp_deviation).exp();

    let win_min = own_min / (own_min + opp_max);
    let mut win_avg = own_avg / (own_avg + opp_avg);
    let win_max = own_max / (own_max + opp_min);

    let delta = win_max - win_min;

    let evaluation = if delta < 0.15 {
        ""
    } else if delta < 0.3 {
        "?"
    } else if delta < 0.6 {
        "??"
    } else {
        win_avg = f64::NAN;
        "???"
    };

    (
        (win_min * 100.0).round(),
        (win_avg * 100.0).round(),
        (win_max * 100.0).round(),
        evaluation,
    )
}

#[derive(Serialize)]
struct PlayerMatchup {
    character: String,
    game_count: i32,
    win_rate_real: f64,
    win_rate_adjusted: f64,
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

pub async fn get_player_char_history(
    conn: &RatingsDbConn,
    id: i64,
    char_id: i64,
    game_count: i64,
    group_games: bool,
) -> Option<PlayerCharacterHistory> {
    conn.run(move |conn| {
        let (_wins, _losses, value, deviation, _global_rank, _character_rank) = conn
            .query_row(
                "SELECT wins, losses, value, deviation, global_rank, character_rank
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
                        row.get::<_, Option<i32>>(4).unwrap(),
                        row.get::<_, Option<i32>>(5).unwrap(),
                    ))
                },
            )
            .unwrap();

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
                            value_b AS opponent_value,
                            deviation_b AS opponent_deviation,
                            winner,
                            vip_status,
                            cheater_status
                        FROM games NATURAL JOIN game_ratings
                        LEFT JOIN vip_status ON vip_status.id = games.id_b
                        LEFT JOIN cheater_status ON cheater_status.id = games.id_b
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
                            winner + 2  as winner,
                            vip_status,
                            cheater_status
                        FROM games NATURAL JOIN game_ratings
                        LEFT JOIN vip_status ON vip_status.id = games.id_a
                        LEFT JOIN cheater_status ON cheater_status.id = games.id_a
                        WHERE games.id_b = :id AND games.char_b = :char_id

                        ORDER BY timestamp DESC LIMIT :game_count",
                )
                .unwrap();

            let mut rows = stmt
                .query(named_params! {
                    ":id" : id,
                    ":char_id": char_id,
                    ":game_count":game_count,
                })
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
                let opponent_vip: Option<String> = row.get("vip_status").unwrap();
                let opponent_cheater: Option<String> = row.get("cheater_status").unwrap();

                merge_set(
                    &mut history,
                    UnmergedPlayerSet {
                        timestamp,
                        floor,
                        own_value,
                        own_deviation,
                        opponent_name,
                        opponent_id,
                        opponent_character,
                        winner,
                        opponent_vip,
                        opponent_cheater,
                        opponent_value,
                        opponent_deviation,
                    },
                    group_games,
                );
            }

            history
        };

        let recent_games = {
            let mut stmt = conn
                .prepare_cached(
                    "SELECT
                            games.timestamp AS timestamp,
                            game_floor,
                            name_b AS opponent_name,
                            games.id_b AS opponent_id,
                            games.char_b AS opponent_character,
                            winner,
                            vip_status,
                            cheater_status
                        FROM games LEFT JOIN game_ratings
                        ON games.id_a = game_ratings.id_a
                            AND games.id_b = game_ratings.id_b
                            AND games.timestamp = game_ratings.timestamp
                        LEFT JOIN vip_status ON vip_status.id = games.id_b
                        LEFT JOIN cheater_status ON cheater_status.id = games.id_b
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
                            winner + 2  as winner,
                            vip_status,
                            cheater_status
                        FROM games LEFT JOIN game_ratings
                        ON games.id_a = game_ratings.id_a
                            AND games.id_b = game_ratings.id_b
                            AND games.timestamp = game_ratings.timestamp
                        LEFT JOIN vip_status ON vip_status.id = games.id_a
                        LEFT JOIN cheater_status ON cheater_status.id = games.id_a
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
                let opponent_vip: Option<String> = row.get("vip_status").unwrap();
                let opponent_cheater: Option<String> = row.get("cheater_status").unwrap();

                let (opponent_value, opponent_deviation) = conn
                    .query_row(
                        "SELECT value, deviation
                        FROM player_ratings
                        WHERE id=? AND char_id=?",
                        params![opponent_id, opponent_character],
                        |row| Ok((row.get::<_, f64>(0).unwrap(), row.get::<_, f64>(1).unwrap())),
                    )
                    .optional()
                    .unwrap()
                    .unwrap_or((0.0, 350.0 / 173.7178));

                merge_set(
                    &mut recent_games,
                    UnmergedPlayerSet {
                        timestamp,
                        floor,
                        own_value: value,
                        own_deviation: deviation,
                        opponent_name,
                        opponent_id,
                        opponent_character,
                        winner,
                        opponent_vip,
                        opponent_cheater,
                        opponent_value,
                        opponent_deviation,
                    },
                    group_games,
                );
            }

            recent_games
        };

        Some(PlayerCharacterHistory {
            history,
            recent_games,
        })
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
            let (name, vip_status, cheater_status): (String, Option<String>, Option<String>) = conn
                .query_row(
                    "SELECT name, vip_status, cheater_status FROM players
                        LEFT JOIN vip_status ON vip_status.id = players.id
                        LEFT JOIN cheater_status ON cheater_status.id = players.id
                           WHERE players.id=?
                           ",
                    params![id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
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
                vip_status,
                cheater_status,
                other_characters,
                other_names,
                data: character_data,
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
        let rating: GlickoRating = Glicko2Rating {
            value: row.get(3).unwrap(),
            deviation: row.get(4).unwrap(),
            volatility: 0.0,
        }
        .into();

        let character_name = website::CHAR_NAMES[char_id].1.to_owned();
        let character_shortname = website::CHAR_NAMES[char_id].0.to_owned();
        other_characters.push(OtherPlayerCharacter {
            character_name,
            character_shortname,
            game_count,
            rating_value: rating.value.round(),
            rating_deviation: (rating.deviation * 2.0).round(),
        });
    }

    other_characters
}

fn get_player_character_data(
    conn: &Connection,
    id: i64,
    char_id: i64,
) -> Result<Option<PlayerCharacterData>> {
    let (wins, losses, value, deviation, global_rank, character_rank) = match conn.query_row(
        "SELECT wins, losses, value, deviation, global_rank, character_rank
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
                row.get::<_, Option<i32>>(4).unwrap(),
                row.get::<_, Option<i32>>(5).unwrap(),
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
                    win_rate_real: (wins_real / (wins_real + losses_real) * 100.0).round(),
                    win_rate_adjusted: (wins_adjusted / (wins_adjusted + losses_adjusted) * 100.0)
                        .round(),
                });
            }

            matchups.sort_by_key(|m| -(m.win_rate_adjusted as i32));

            matchups
        };

        Ok(Some(PlayerCharacterData {
            character_name,
            game_count: wins + losses,
            win_rate: wins as f64 / (wins + losses) as f64,
            rating_value: (value * 173.7178 + 1500.0).round(),
            rating_deviation: (deviation * 173.7178 * 2.0).round(),
            matchups,
            character_rank,
            global_rank,
        }))
    }
}

struct UnmergedPlayerSet {
    timestamp: i64,
    floor: i64,
    own_value: f64,
    own_deviation: f64,
    opponent_name: String,
    opponent_id: i64,
    opponent_character: i64,
    opponent_value: f64,
    opponent_deviation: f64,
    winner: i64,
    opponent_vip: Option<String>,
    opponent_cheater: Option<String>,
}

fn merge_set(sets: &mut Vec<PlayerSet>, set: UnmergedPlayerSet, group_games: bool) {
    let UnmergedPlayerSet {
        timestamp,
        floor,
        own_value,
        own_deviation,
        opponent_name,
        opponent_id,
        opponent_character,
        winner,
        opponent_vip,
        opponent_cheater,
        opponent_value,
        opponent_deviation,
    } = set;

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

    let (expected_outcome_min, expected_outcome, expected_outcome_max, expected_outcome_evaluation) =
        get_expected_outcomes(own_value, own_deviation, opponent_value, opponent_deviation);

    if let Some(set) = sets.last_mut().filter(|set| {
        set.opponent_id == format!("{:X}", opponent_id)
            && set.opponent_character == website::CHAR_NAMES[opponent_character as usize].1
            && group_games
    }) {
        set.timestamp = format!(
            "{}",
            NaiveDateTime::from_timestamp(timestamp, 0).format("%Y-%m-%d %H:%M")
        );
        set.own_rating_value = own_rating.value.round();
        set.own_rating_deviation = (own_rating.deviation * 2.0).round();
        set.opponent_rating_value = opponent_rating.value.round();
        set.opponent_rating_deviation = (opponent_rating.deviation * 2.0).round();

        set.expected_outcome = expected_outcome;
        set.expected_outcome_evaluation = expected_outcome_evaluation;
        set.expected_outcome_min = expected_outcome_min;
        set.expected_outcome_max = set.expected_outcome_max;

        match winner {
            1 | 4 => set.result_wins += 1,
            2 | 3 => set.result_losses += 1,
            _ => panic!("Bad winner"),
        }

        set.result_percent =
            ((set.result_wins as f64 / (set.result_wins + set.result_losses) as f64) * 100.0)
                .round();
    } else {
        sets.push(PlayerSet {
            timestamp: format!(
                "{}",
                NaiveDateTime::from_timestamp(timestamp, 0).format("%Y-%m-%d %H:%M")
            ),
            own_rating_value: own_rating.value.round(),
            own_rating_deviation: (own_rating.deviation * 2.0).round(),
            floor: match floor {
                99 => format!("Celestial"),
                n => format!("Floor {}", n),
            },
            opponent_name: opponent_name,
            opponent_vip,
            opponent_cheater,
            opponent_id: format!("{:X}", opponent_id),
            opponent_character: website::CHAR_NAMES[opponent_character as usize]
                .1
                .to_owned(),
            opponent_character_short: website::CHAR_NAMES[opponent_character as usize]
                .0
                .to_owned(),
            opponent_rating_value: opponent_rating.value.round(),
            opponent_rating_deviation: (opponent_rating.deviation * 2.0).round(),
            expected_outcome,
            expected_outcome_evaluation,
            expected_outcome_min,
            expected_outcome_max,
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
            .query_row("SELECT SUM(player_count) FROM player_floor_distribution", [], |r| r.get(0))
            .unwrap();

        let total_games: i64 = conn
            .query_row("SELECT SUM(game_count) FROM player_floor_distribution", [], |r| r.get(0))
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
                average_offset: format!("{:+.1}", (row.get::<_, f64>(2).unwrap() * 173.7178)),
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
                average_offset: format!("{:+.1}", (row.get::<_, f64>(2).unwrap() * 173.7178)),
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
                average_offset: format!("{:+.1}", (row.get::<_, f64>(2).unwrap() * 173.7178)),
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
                .query(params![rater::MAX_DEVIATION, rater::MAX_DEVIATION, id, id,])
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
                let a = a * 173.718 + 1500.0;
                let b = b * 173.718 + 1500.0;

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
                    rater::MAX_DEVIATION,
                    rater::MAX_DEVIATION,
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

            let mut rows = stmt.query(params![rater::MAX_DEVIATION]).unwrap();

            let mut totals: FxHashMap<i64, FxHashMap<i64, i64>> = Default::default();
            let mut overall: FxHashMap<i64, i64> = Default::default();

            while let Some(row) = rows.next().unwrap() {
                let floor: i64 = row.get(0).unwrap();
                let value: f64 = row.get(1).unwrap();
                let value = value * 173.718 + 1500.0;

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
