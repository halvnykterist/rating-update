use crate::{ggst_api, glicko, glicko::Rating, responses, website};
use anyhow::Context;
use chrono::{NaiveDateTime, Utc};
use fxhash::{FxHashMap, FxHashSet};
use lazy_static::lazy_static;
use rusqlite::{
    functions::FunctionFlags, named_params, params, Connection, OptionalExtension, Row, Transaction,
};
use std::{sync::Mutex, time::Duration};
use tokio::{time, try_join};

const DECAY_CONSTANT: f64 = 3.1;

pub const LOW_DEVIATION: f64 = 75.0;
pub const HIGH_RATING: f64 = 1800.0;
pub const DB_NAME: &str = "ratings.sqlite";

const CHAR_COUNT: usize = website::CHAR_NAMES.len();
pub const POP_RATING_BRACKETS: usize = 13;

pub const RATING_PERIOD: i64 = 60 * 60;
pub const RANKING_PERIOD: i64 = 1 * 60 * 60;
pub const STATISTICS_PERIOD: i64 = 24 * 60 * 60;

lazy_static! {
    pub static ref RUNTIME_DATA: Mutex<RuntimeData> = Mutex::new(RuntimeData {});
}

pub struct RuntimeData {}

type Result<T> = std::result::Result<T, anyhow::Error>;

pub fn init_database() -> Result<()> {
    info!("Intializing database");

    let conn = Connection::open(DB_NAME)?;
    conn.execute_batch(include_str!("../init.sql"))?;

    Ok(())
}

pub fn reset_database() -> Result<()> {
    info!("Resetting database");
    let conn = Connection::open(DB_NAME)?;
    conn.execute_batch(include_str!("../reset.sql"))?;

    Ok(())
}

pub fn reset_names() -> Result<()> {
    let mut conn = Connection::open(DB_NAME)?;

    let tx = conn.transaction()?;

    let games = {
        let mut stmt = tx
            .prepare("SELECT * FROM games ORDER BY timestamp ASC")
            .unwrap();

        let mut rows = stmt.query([]).unwrap();
        let mut games = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            games.push(Game::from_row(row));
        }
        games
    };

    for g in games {
        update_player(&tx, g.id_a, &g.name_a, g.game_floor, g.platform_a);
        update_player(&tx, g.id_b, &g.name_b, g.game_floor, g.platform_b);
    }

    tx.commit()?;

    Ok(())
}

pub fn reset_distribution() -> Result<()> {
    let mut conn = Connection::open(DB_NAME)?;

    update_player_distribution(&mut conn);

    Ok(())
}

pub async fn run() -> Result<()> {
    try_join! {
        async {
            tokio::spawn(pull_continuous()).await?;
            Ok(())
        },
        async {
            tokio::spawn(
                async {
                    update_statistics_continuous()
                    .await
                    .context("Inside `update_rating_continuous`")
                }).await?
        },
    }?;

    Ok(())
}

async fn pull_continuous() {
    let mut conn = Connection::open(DB_NAME).unwrap();
    grab_games(&mut conn, 100).await.unwrap();
    let mut interval = time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        if let Err(e) = grab_games(&mut conn, 10).await {
            error!("grab_games failed: {}", e)
        }
    }
}

pub async fn update_statistics_continuous() -> Result<()> {
    let mut conn = Connection::open(DB_NAME)?;

    let mut last_ranking_update: i64 =
        conn.query_row("SELECT last_update FROM config", [], |r| r.get(0))?;
    let mut last_statistics_update = last_ranking_update;

    let mut interval = time::interval(Duration::from_secs(60));

    loop {
        interval.tick().await;
        let now = Utc::now().timestamp();
        if now - last_ranking_update > RANKING_PERIOD {
            info!("New ranking period, updating decay and rankings");

            if last_ranking_update - last_statistics_update >= STATISTICS_PERIOD {
                info!("New statistics period, updating statistics.");
                last_statistics_update = last_ranking_update;
                update_player_distribution(&mut conn);
                //if let Err(e) = calc_versus_matchups(&mut conn) {
                //    error!("calc_versus_matchups failed: {}", e);
                //}
                if let Err(e) = calc_fraud_index(&mut conn) {
                    error!("calc_fraud_index failed: {}", e);
                }
                if let Err(e) = calc_character_popularity(&mut conn, last_ranking_update) {
                    error!("calc_character_popularity failed: {}", e);
                }
            }

            if let Err(e) = update_decay(&mut conn, Utc::now().timestamp()) {
                error!("update_decay failed: {}", e);
            }
            if let Err(e) = decay_matchups(&mut conn, Utc::now().timestamp()) {
                error!("decay_matchups failed: {}", e);
            }
            if let Err(e) = update_rankings(&mut conn) {
                error!("update_rankings failed: {}", e);
            }

            while now - last_ranking_update > RANKING_PERIOD {
                last_ranking_update += RANKING_PERIOD;
            }

            info!(
                "Last ranking period: {}",
                NaiveDateTime::from_timestamp(last_ranking_update, 0)
            );

            conn.execute(
                "UPDATE config SET last_update = ?",
                params![last_ranking_update],
            )
            .unwrap();
        }
    }
}

pub async fn update_once() {
    let mut conn = Connection::open(DB_NAME).unwrap();

    while update_ratings(&mut conn, None) > 0 {
        update_rankings(&mut conn).unwrap();
    }

    //let last_rating_timestamp: i64 = conn
    //    .query_row("SELECT last_update FROM config", [], |r| r.get(0))
    //    .unwrap();
    update_player_distribution(&mut conn);
    //if let Err(e) = calc_versus_matchups(&mut conn) {
    //    error!("calc_versus_matchups failed: {}", e);
    //}
    if let Err(e) = calc_fraud_index(&mut conn) {
        error!("calc_fraud_index failed: {}", e);
    }

    if let Err(e) = update_rankings(&mut conn) {
        error!("update_rankings failed: {}", e);
    }
    //if let Err(e) = calc_character_popularity(&mut conn, last_rating_timestamp) {
    //    error!("calc_character_popularity failed: {}", e);
    //}
}

pub fn print_rankings() {
    let conn = Connection::open(DB_NAME).unwrap();

    println!("| Rank | Name | Character | Rating | Games |");
    println!("|------|------|-----------|--------|-------|");

    let mut stmt = conn
            .prepare(
                "SELECT name, char_id, value, deviation, (wins + losses) as games, (100.0 * wins) / (wins + losses) as win_rate
                FROM player_ratings NATURAL JOIN players
                WHERE deviation < 75.0 
                ORDER BY value - 2.0 * deviation DESC
                LIMIT 100
                ",
            )
            .unwrap();

    let mut rank = 1;
    let mut rows = stmt.query([]).unwrap();

    while let Some(row) = rows.next().unwrap() {
        let name: String = row.get(0).unwrap();
        let char_name = website::CHAR_NAMES[row.get::<_, usize>(1).unwrap()].1;
        let value: f64 = row.get(2).unwrap();
        let deviation: f64 = row.get(3).unwrap();
        let games: i64 = row.get(4).unwrap();
        let rate: f64 = row.get(5).unwrap();
        println!(
            "| {} | {} | {} | {:.0} ±{:.0} | {} ({:.0}%) |",
            rank, name, char_name, value, deviation, games, rate
        );

        rank += 1;
    }
}

pub fn mark_vip(vip_id: &str, notes: &str) {
    let vip_id = i64::from_str_radix(vip_id, 16).unwrap();

    let conn = Connection::open(DB_NAME).unwrap();
    conn.execute(
        "INSERT INTO vip_status
            VALUES(?, 'VIP', ?)",
        params![vip_id, notes],
    )
    .unwrap();
}

pub fn mark_hidden(hidden_id: &str, notes: &str) {
    let hidden_id = i64::from_str_radix(hidden_id, 16).unwrap();

    let conn = Connection::open(DB_NAME).unwrap();
    conn.execute(
        "INSERT INTO hidden_status
            VALUES(?, 'hidden', ?)",
        params![hidden_id, notes],
    )
    .unwrap();
}

pub async fn mark_cheater(
    cheater_id: Option<&str>,
    cheater_type: Option<&str>,
    notes: Option<&str>,
) {
    let cheater_id = i64::from_str_radix(cheater_id.unwrap(), 16).unwrap();

    let conn = Connection::open(DB_NAME).unwrap();

    struct Game {
        id_a: i64,
        char_a: i64,
        value_a: f64,
        deviation_a: f64,
        id_b: i64,
        char_b: i64,
        value_b: f64,
        deviation_b: f64,
        winner: i64,
    }

    let games = {
        let mut stmt = conn.prepare(
            "SELECT id_a, char_a, value_a, deviation_a, id_b, char_b, value_b, deviation_b, winner
            FROM game_ratings
            NATURAL JOIN games
            WHERE id_a = ? OR id_b = ?").unwrap();

        let mut games = Vec::new();
        let mut rows = stmt.query(params![cheater_id, cheater_id]).unwrap();

        while let Some(row) = rows.next().unwrap() {
            games.push(Game {
                id_a: row.get(0).unwrap(),
                char_a: row.get(1).unwrap(),
                value_a: row.get(2).unwrap(),
                deviation_a: row.get(3).unwrap(),
                id_b: row.get(4).unwrap(),
                char_b: row.get(5).unwrap(),
                value_b: row.get(6).unwrap(),
                deviation_b: row.get(7).unwrap(),
                winner: row.get(8).unwrap(),
            });
        }

        games
    };

    let mut player_offsets = FxHashMap::<(i64, i64), f64>::default();

    for g in games {
        if g.id_a == cheater_id {
            let change = Rating::new(g.value_b, g.deviation_b).rating_change(
                Rating::new(g.value_a, g.deviation_a),
                if g.winner == 1 { 0.0 } else { 1.0 },
            );

            *player_offsets.entry((g.id_b, g.char_b)).or_default() -= change;
        } else {
            let change = Rating::new(g.value_a, g.deviation_a).rating_change(
                Rating::new(g.value_b, g.deviation_b),
                if g.winner == 1 { 1.0 } else { 0.0 },
            );

            *player_offsets.entry((g.id_a, g.char_a)).or_default() -= change;
        }
    }

    for (key, value) in &player_offsets {
        println!("{:?}: {:.1}", key, value);
    }

    if cheater_type.is_some() {
        for ((id, char_id), offset) in player_offsets {
            conn.execute(
                "UPDATE player_ratings 
            SET value = value + ?
            WHERE id= ? AND char_id = ?",
                params![offset, id, char_id],
            )
            .unwrap();
        }

        conn.execute(
            "INSERT INTO cheater_status
            VALUES(?, ?, ?)",
            params![cheater_id, cheater_type, notes.unwrap_or("")],
        )
        .unwrap();
    }
}

pub async fn update_fraud_once() {
    let mut conn = Connection::open(DB_NAME).unwrap();

    if let Err(e) = calc_fraud_index(&mut conn) {
        error!("calc_fraud_index failed: {}", e);
    }
}

pub async fn update_decay_once() {
    let mut conn = Connection::open(DB_NAME).unwrap();

    update_decay(&mut conn, Utc::now().timestamp()).unwrap();
}

pub fn get_average_rating(conn: &Transaction, id: i64) -> f64 {
    conn.query_row(
        "select avg(value) from player_ratings where id = ?",
        params![id],
        |r| r.get::<_, Option<f64>>(0),
    )
    .unwrap()
    .unwrap_or_default()
}

pub async fn pull() {
    let mut conn = Connection::open(DB_NAME).unwrap();

    grab_games(&mut conn, 100).await.unwrap();
}

async fn grab_games(conn: &mut Connection, _pages: usize) -> Result<()> {
    let then = Utc::now();
    info!("Grabbing replays");
    let replays = ggst_api::get_replays().await;

    let replays = match replays {
        Ok(replays) => replays,
        Err(e) => {
            error!("Error fetching replays: {e}");
            return Ok(());
        }
    };

    let old_count: i64 = conn.query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))?;

    let tx = conn.transaction()?;

    let mut new_games = Vec::new();

    let num_replays = replays.len();
    for r in replays {
        new_games.extend(add_game(&tx, r));
    }
    tx.commit()?;

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM games", [], |r| r.get(0))?;

    let elapsed = (Utc::now() - then).num_milliseconds();

    info!(
        "Grabbed {} games -  new games: {} ({} total) - {}ms",
        num_replays,
        count - old_count,
        count,
        elapsed,
    );

    assert_eq!(count - old_count, new_games.len() as i64);

    update_ratings(conn, Some(new_games));

    if count - old_count == num_replays as i64 {
        if num_replays > 0 {
            error!("Only new replays! We're probably missing some, try increasing the page count.");
        } else {
            error!("No replays! Maybe servers are down?");
        }
    } else if count - old_count > num_replays as i64 / 2 {
        warn!("Over half the grabbed replays are new, consider increasing page count.");
    }

    Ok(())
}

fn add_game(conn: &Transaction, game: responses::Replay) -> Option<Game> {
    //2023-01-30 01:52:15"
    let responses::Replay {
        timestamp,
        player1,
        player1_character,
        player2,
        player2_character,
        floor: game_floor,
        winner,
        ..
    } = game;
    let timestamp = NaiveDateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M:%S").unwrap();

    let count = conn
        .execute(
            "INSERT OR IGNORE INTO games (
            timestamp,
            id_a,
            name_a,
            char_a,
            platform_a,
            id_b,
            name_b,
            char_b,
            platform_b,
            winner,
            game_floor
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                timestamp.timestamp(),
                player1.id,
                player1.name,
                player1_character,
                player1.platform,
                player2.id,
                player2.name,
                player2_character,
                player2.platform,
                winner,
                game_floor,
            ],
        )
        .unwrap();

    if count == 1 {
        Some(Game {
            timestamp: timestamp.timestamp(),
            id_a: player1.id.parse().unwrap(),
            char_a: player1_character,
            platform_a: player1.platform,
            name_a: player1.name,
            id_b: player2.id.parse().unwrap(),
            char_b: player2_character,
            name_b: player2.name,
            platform_b: player2.platform,
            winner,
            game_floor,
        })
    } else {
        None
    }

    //Check if it already exists in the db
    //if it doesn't, add it to the list of things to calculate ratings based on

    //sort the list by date
}

fn update_player(conn: &Transaction, id: i64, name: &str, floor: i64, platform: i64) {
    if let Err(e) = conn.execute(
        "REPLACE INTO players(id, name, floor, platform) VALUES(?, ?, ?, ?)",
        params![id, name, floor, platform],
    ) {
        warn!("{}", e);
    }

    if let Err(e) = conn.execute(
        "INSERT OR IGNORE INTO player_names(id, name) VALUES(?, ?)",
        params![id, name],
    ) {
        warn!("{}", e);
    }
}

fn update_player_distribution(conn: &mut Connection) {
    let then = Utc::now();
    let tx = conn.transaction().unwrap();

    let two_weeks_ago = then.timestamp() - 60 * 60 * 24 * 14;

    tx.execute("DELETE FROM player_floor_distribution", [])
        .unwrap();
    tx.execute("DELETE FROM player_rating_distribution", [])
        .unwrap();

    for f in (1..=10).chain(std::iter::once(99)) {
        let player_count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM players WHERE floor = ?",
                params![f],
                |r| r.get(0),
            )
            .unwrap();
        let game_count: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM games WHERE game_floor = ? AND timestamp > ?",
                params![f, two_weeks_ago],
                |r| r.get(0),
            )
            .unwrap();

        tx.execute(
            "INSERT INTO
            player_floor_distribution
            (floor, player_count, game_count)
            VALUES (?, ?, ?)",
            params![f, player_count, game_count],
        )
        .unwrap();
    }

    for r in 0..600 {
        let r_min = r * 50;
        let r_max = (r + 1) * 50;

        let player_count: i64 = tx
            .query_row(
                "SELECT COUNT(*)
                FROM player_ratings
                WHERE value >= ? AND value < ? AND deviation < ?",
                params![r_min as f64, r_max as f64, LOW_DEVIATION],
                |r| r.get(0),
            )
            .unwrap();

        if player_count < 10 {
            continue;
        }

        let player_count_cum: i64 = tx
            .query_row(
                "SELECT COUNT(*)
                FROM player_ratings
                WHERE value < ? AND deviation < ?",
                params![r_max as f64, LOW_DEVIATION],
                |r| r.get(0),
            )
            .unwrap();

        tx.execute(
            "INSERT INTO
            player_rating_distribution
            (min_rating, max_rating, player_count, player_count_cum)
            VALUES (?, ?, ?, ?)",
            params![r_min, r_max, player_count, player_count_cum],
        )
        .unwrap();
    }

    tx.commit().unwrap();

    info!(
        "Updated player distribution - {}ms",
        (Utc::now() - then).num_milliseconds()
    );
}

fn update_ratings(conn: &mut Connection, games: Option<Vec<Game>>) -> i64 {
    info!("Updating ratings");
    let then = Utc::now();

    let tx = conn.transaction().unwrap();
    //Fetch the games from the rating period
    let (games, remaining) = games.map(|g| (g, 0)).unwrap_or_else(|| {
        let mut stmt = tx
            .prepare(
                "SELECT
                    games.timestamp,
                    games.id_a,
                    games.name_a,
                    games.char_a,
                    games.platform_a,
                    games.id_b,
                    games.name_b,
                    games.char_b,
                    games.platform_b,
                    games.winner,
                    games.game_floor
                FROM
                    games LEFT JOIN game_ratings ON
                    games.id_a == game_ratings.id_a
                    AND games.id_b == game_ratings.id_b
                    AND games.timestamp == game_ratings.timestamp
                WHERE game_ratings.id_a IS NULL
                ORDER BY games.timestamp ASC
                LIMIT 250000",
            )
            .unwrap();

        let mut rows = stmt.query([]).unwrap();
        let mut games = Vec::new();
        while let Some(row) = rows.next().unwrap() {
            games.push(Game::from_row(row));
        }

        let remaining = {
            let mut stmt = tx
                .prepare(
                    "SELECT COUNT(*)
                FROM
                    games LEFT JOIN game_ratings ON
                    games.id_a == game_ratings.id_a
                    AND games.id_b == game_ratings.id_b
                    AND games.timestamp == game_ratings.timestamp
                WHERE game_ratings.id_a IS NULL",
                )
                .unwrap();

            let count: i64 = stmt.query_row(params![], |r| r.get(0)).unwrap();
            count
        };

        info!(
            "Fetched {} games to rate from {} remaining - {}ms",
            games.len(),
            remaining,
            (Utc::now() - then).num_milliseconds(),
        );
        (games, remaining)
    });

    //let popularities =

    //Fetch all the players in the games
    let mut players = FxHashMap::default();
    for g in &games {
        if !players.contains_key(&(g.id_a, g.char_a)) {
            players.insert(
                (g.id_a, g.char_a),
                tx.query_row(
                    "SELECT 
                        player_ratings.id, player_ratings.char_id, wins, losses, value, deviation, last_decay,
                        top_rating_value, top_rating_deviation, top_rating_timestamp,
                        top_defeated_id, top_defeated_char_id, top_defeated_name,
                        top_defeated_value, top_defeated_deviation, top_defeated_floor,
                        top_defeated_timestamp, character_rank
                    FROM player_ratings LEFT JOIN ranking_character 
                    ON 
                        player_ratings.id = ranking_character.id AND 
                        player_ratings.char_id = ranking_character.char_id
                    WHERE player_ratings.id = ? AND player_ratings.char_id = ?",
                    params![g.id_a, g.char_a],
                    |r| Ok(RatedPlayer::from_row(r)),
                )
                .optional()
                .unwrap()
                .unwrap_or(RatedPlayer::new(g.id_a, g.char_a, g.timestamp)),
            );
        }
        if !players.contains_key(&(g.id_b, g.char_b)) {
            players.insert(
                (g.id_b, g.char_b),
                tx.query_row(
                    "SELECT 
                        player_ratings.id, player_ratings.char_id, wins, losses, value, deviation, last_decay,
                        top_rating_value, top_rating_deviation, top_rating_timestamp,
                        top_defeated_id, top_defeated_char_id, top_defeated_name,
                        top_defeated_value, top_defeated_deviation, top_defeated_floor,
                        top_defeated_timestamp, character_rank
                    FROM player_ratings LEFT JOIN ranking_character 
                    ON 
                        player_ratings.id = ranking_character.id AND 
                        player_ratings.char_id = ranking_character.char_id
                    WHERE player_ratings.id = ? AND player_ratings.char_id = ?",
                    params![g.id_b, g.char_b],
                    |r| Ok(RatedPlayer::from_row(r)),
                )
                .optional()
                .unwrap()
                .unwrap_or(RatedPlayer::new(g.id_b, g.char_b, g.timestamp)),
            );
        }
    }

    info!("Fetched {} players", players.len());

    //fetch all our known cheaters
    let cheaters = {
        let mut cheaters = FxHashSet::<i64>::default();

        let mut stmt = tx
            .prepare(
                "SELECT
                    id
                FROM cheater_status",
            )
            .unwrap();
        let mut rows = stmt.query([]).unwrap();

        while let Some(row) = rows.next().unwrap() {
            cheaters.insert(row.get(0).unwrap());
        }
        cheaters
    };

    let mut counter = 0;

    //let mut last_timestamp = 0;

    let popularities = {
        let mut stmt = tx
            .prepare("SELECT char_id, popularity FROM character_popularity_global")
            .unwrap();

        let mut rows = stmt.query([]).unwrap();

        let mut popularities = FxHashMap::<i64, f64>::default();

        while let Some(row) = rows.next().unwrap() {
            popularities.insert(row.get(0).unwrap(), row.get(1).unwrap());
        }

        popularities
    };

    for g in games {
        //This fails and I don't know why
        //assert_ge!(g.timestamp, last_timestamp);
        //last_timestamp = g.timestamp;

        counter += 1;
        if counter % 50_000 == 0 {
            info!("On game {}...", counter);
        }

        update_player(&tx, g.id_a, &g.name_a, g.game_floor, g.platform_a);
        update_player(&tx, g.id_b, &g.name_b, g.game_floor, g.platform_b);

        let has_cheater = cheaters.contains(&g.id_a) || cheaters.contains(&g.id_b);

        let old_rating_a = players.get(&(g.id_a, g.char_a)).unwrap().rating;
        let old_rating_b = players.get(&(g.id_b, g.char_b)).unwrap().rating;

        players
            .get_mut(&(g.id_a, g.char_a))
            .unwrap()
            .decay(g.timestamp);
        players
            .get_mut(&(g.id_b, g.char_b))
            .unwrap()
            .decay(g.timestamp);

        let (winner, loser) = match g.winner {
            1 => ((g.id_a, g.char_a), (g.id_b, g.char_b)),
            2 => ((g.id_b, g.char_b), (g.id_a, g.char_a)),
            _ => panic!("Bad winner"),
        };

        let winner_rating = players.get(&winner).unwrap().rating;
        let winner_rank = players
            .get(&winner)
            .unwrap()
            .character_rank
            .unwrap_or(99999);
        let winner_char = players.get(&winner).unwrap().char_id;
        let loser_rating = players.get(&loser).unwrap().rating;
        let loser_rank = players.get(&loser).unwrap().character_rank.unwrap_or(99999);
        let loser_char = players.get(&loser).unwrap().char_id;

        let expected_outcome = winner_rating.expected(loser_rating);

        const MARGIN: f64 = 0.045;
        let rsm_deviation = (0.5 * winner_rating.deviation.powf(2.0)
            + 0.5 * loser_rating.deviation.powf(2.0))
        .sqrt();
        let valid = ((expected_outcome > MARGIN && expected_outcome < 1.0 - MARGIN)
            || rsm_deviation >= 50.0)
            && !has_cheater;

        if valid {
            //Update ratings
            players.get_mut(&winner).unwrap().rating = winner_rating.update(loser_rating, 1.0);
            players.get_mut(&winner).unwrap().win_count += 1;

            players.get_mut(&loser).unwrap().rating = loser_rating.update(winner_rating, 0.0);
            players.get_mut(&loser).unwrap().loss_count += 1;

            //Update top rating and top defeated
            players
                .get_mut(&winner)
                .unwrap()
                .update_top_rating(g.timestamp);

            let loser_name = match g.winner {
                1 => g.name_b,
                2 => g.name_a,
                _ => panic!("Bad winner"),
            };
            players.get_mut(&winner).unwrap().update_top_defeated(
                loser.0,
                loser.1,
                loser_name.to_owned(),
                loser_rating,
                g.game_floor,
                g.timestamp,
            );
            players
                .get_mut(&loser)
                .unwrap()
                .update_top_rating(g.timestamp);

            //Update player matchups
            fn update_player_matchup(
                tx: &Transaction,
                player_id: i64,
                char_id: i64,
                player_rating: Rating,
                opp_char_id: i64,
                opp_rating: Rating,
                result: f64,
                game_timestamp: i64,
            ) {
                tx.execute(
                    "INSERT OR IGNORE INTO player_matchups VALUES(?, ?, ?, ?, 350.0, ?, 0, 0)",
                    params![
                        player_id,
                        char_id,
                        opp_char_id,
                        player_rating.value,
                        game_timestamp
                    ],
                )
                .unwrap();

                let (value, deviation, mut last_decay_timestamp, mut wins, mut losses): (
                    f64,
                    f64,
                    i64,
                    i64,
                    i64,
                ) = tx
                    .query_row(
                        "SELECT 
                            rating_value, 
                            rating_deviation, 
                            rating_timestamp, 
                            wins,
                            losses
                        FROM player_matchups
                        WHERE id=? AND char_id=? AND opp_char_id=?",
                        params![player_id, char_id, opp_char_id],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
                    )
                    .unwrap();

                let mut rating = Rating::new(value, deviation);

                rating = rating.update(opp_rating, result);

                while last_decay_timestamp + RATING_PERIOD < game_timestamp {
                    rating.decay_deviation(1, DECAY_CONSTANT);
                    last_decay_timestamp += RATING_PERIOD;
                }

                if result == 1.0 {
                    wins += 1;
                } else {
                    losses += 1;
                }

                let Rating { value, deviation } = rating;

                tx.execute(
                    "UPDATE player_matchups
                SET 
                    rating_value = ?,
                    rating_deviation = ?,
                    rating_timestamp = ?,
                    wins = ?,
                    losses = ?
                WHERE id=? AND char_id=? AND opp_char_id=?",
                    params![
                        value,
                        deviation,
                        last_decay_timestamp,
                        wins,
                        losses,
                        player_id,
                        char_id,
                        opp_char_id
                    ],
                )
                .unwrap();
            }

            update_player_matchup(
                &tx,
                winner.0,
                winner.1,
                winner_rating,
                loser.1,
                loser_rating,
                1.0,
                g.timestamp,
            );
            update_player_matchup(
                &tx,
                loser.0,
                loser.1,
                loser_rating,
                winner.1,
                winner_rating,
                0.0,
                g.timestamp,
            );

            fn update_global_matchup(
                tx: &Transaction,
                table: &str,
                winner_char: i64,
                loser_char: i64,
            ) {
                tx.execute(
                    &format!(
                        "INSERT OR IGNORE INTO {} VALUES(?, ?, 1500.0, 350.0, 0, 0)",
                        table
                    ),
                    params![winner_char, loser_char,],
                )
                .unwrap();
                tx.execute(
                    &format!(
                        "INSERT OR IGNORE INTO {} VALUES(?, ?, 1500.0, 350.0, 0, 0)",
                        table
                    ),
                    params![loser_char, winner_char],
                )
                .unwrap();

                let (winner_value, winner_deviation, mut winner_wins): (f64, f64, i64) = tx
                    .query_row(
                        &format!(
                            "SELECT 
                        rating_value, rating_deviation, wins
                    FROM {}
                    WHERE char_id = ? AND opp_char_id = ?",
                            table
                        ),
                        params![winner_char, loser_char],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .unwrap();

                let (loser_value, loser_deviation, mut loser_losses): (f64, f64, i64) = tx
                    .query_row(
                        &format!(
                            "SELECT 
                        rating_value, rating_deviation, losses
                    FROM {}
                    WHERE char_id = ? AND opp_char_id  = ?",
                            table
                        ),
                        params![loser_char, winner_char],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                    )
                    .unwrap();

                let new_winner = Rating::new(winner_value, winner_deviation).update_with_min_dev(
                    Rating::new(loser_value, loser_deviation),
                    1.0,
                    5.0,
                );
                let new_loser = Rating::new(loser_value, loser_deviation).update_with_min_dev(
                    Rating::new(winner_value, winner_deviation),
                    0.0,
                    5.0,
                );

                winner_wins += 1;
                loser_losses += 1;

                tx.execute(
                    &format!(
                        "UPDATE {}
                SET 
                    rating_value=?,
                    rating_deviation=?,
                    wins=?
                WHERE
                    char_id = ? AND opp_char_id = ?",
                        table
                    ),
                    params![
                        new_winner.value,
                        new_winner.deviation,
                        winner_wins,
                        winner_char,
                        loser_char,
                    ],
                )
                .unwrap();

                tx.execute(
                    &format!(
                        "UPDATE {}
                SET 
                    rating_value=?,
                    rating_deviation=?,
                    losses=?
                WHERE
                    char_id = ? AND opp_char_id = ?",
                        table
                    ),
                    params![
                        new_loser.value,
                        new_loser.deviation,
                        loser_losses,
                        loser_char,
                        winner_char,
                    ],
                )
                .unwrap();
            }

            update_global_matchup(&tx, "global_matchups", winner.1, loser.1);
            if winner_rank <= 100 && loser_rank <= 100 {
                update_global_matchup(&tx, "top_100_matchups", winner.1, loser.1);
            }
            if winner_rank <= 1000 && loser_rank <= 1000 {
                update_global_matchup(&tx, "top_1000_matchups", winner.1, loser.1);
            }
            if winner_rank as f64 <= popularities.get(&winner_char).unwrap_or(&0.0) * 1000.0
                && loser_rank as f64 <= popularities.get(&loser_char).unwrap_or(&0.0) * 1000.0
            {
                update_global_matchup(&tx, "proportional_matchups", winner.1, loser.1);
            }

            //Update daily ratings
            {
                let day_timestamp = NaiveDateTime::from_timestamp(g.timestamp, 0)
                    .date()
                    .and_hms(0, 0, 0)
                    .timestamp();

                let winner_new_rating = players.get(&winner).unwrap().rating;
                let loser_new_rating = players.get(&loser).unwrap().rating;

                if winner_new_rating.deviation < LOW_DEVIATION {
                    tx.execute(
                        "REPLACE INTO daily_ratings VALUES(?, ?, ?, ?, ?)",
                        params![
                            winner.0,
                            winner.1,
                            day_timestamp,
                            winner_new_rating.value,
                            winner_new_rating.deviation
                        ],
                    )
                    .unwrap();
                }

                if loser_new_rating.deviation < LOW_DEVIATION {
                    tx.execute(
                        "REPLACE INTO daily_ratings VALUES(?, ?, ?, ?, ?)",
                        params![
                            loser.0,
                            loser.1,
                            day_timestamp,
                            loser_new_rating.value,
                            loser_new_rating.deviation
                        ],
                    )
                    .unwrap();
                }
            }
        }

        tx.execute(
            "INSERT INTO game_ratings VALUES(?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                g.timestamp,
                g.id_a,
                old_rating_a.value,
                old_rating_a.deviation,
                g.id_b,
                old_rating_b.value,
                old_rating_b.deviation,
                g.winner,
                valid,
            ],
        )
        .unwrap();
    }

    for (_, player) in players.into_iter() {
        if player.rating.deviation < 0.0 {
            error!("Negative rating deviation???");
        }

        tx.execute(
            "REPLACE INTO player_ratings VALUES(
                ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, 
                ?, ?, ?, ?, ?, ?, ?)",
            params![
                player.id,
                player.char_id,
                player.win_count,
                player.loss_count,
                player.rating.value,
                player.rating.deviation,
                player.last_decay,
                //
                player.top_rating.as_ref().map(|r| r.value),
                player.top_rating.as_ref().map(|r| r.deviation),
                player.top_rating.as_ref().map(|r| r.timestamp),
                //
                player.top_defeated.as_ref().map(|t| t.id),
                player.top_defeated.as_ref().map(|t| t.char_id),
                player.top_defeated.as_ref().map(|t| t.name.clone()),
                player.top_defeated.as_ref().map(|t| t.value),
                player.top_defeated.as_ref().map(|t| t.deviation),
                player.top_defeated.as_ref().map(|t| t.floor),
                player.top_defeated.as_ref().map(|t| t.timestamp),
            ],
        )
        .unwrap();
    }

    tx.commit().unwrap();

    info!(
        "Calculated ratings - {}ms",
        (Utc::now() - then).num_milliseconds()
    );

    remaining
}

pub fn calc_character_popularity(conn: &mut Connection, last_timestamp: i64) -> Result<()> {
    let then = Utc::now();
    info!("Calculating character popularity stats..");
    let one_week_ago = last_timestamp - 60 * 60 * 24 * 7;

    let tx = conn.transaction()?;
    info!("making temp table");
    tx.execute("DROP TABLE IF EXISTS temp.recent_games", [])?;
    tx.execute(
        "CREATE TEMP TABLE temp.recent_games AS
        SELECT
            char_a,
            value_a,
            deviation_a,
            char_b,
            value_b,
            deviation_b
        FROM
            games NATURAL JOIN game_ratings
        WHERE timestamp > ? AND (deviation_a < ? OR deviation_b < ?)",
        params![one_week_ago, LOW_DEVIATION, LOW_DEVIATION],
    )?;
    info!("making indices");
    tx.execute("CREATE INDEX temp.i_char_a ON recent_games(char_a)", [])?;
    tx.execute("CREATE INDEX temp.i_char_b ON recent_games(char_b)", [])?;
    tx.commit()?;
    info!("indices made");

    let tx = conn.transaction()?;

    tx.execute("DELETE FROM character_popularity_global", [])?;
    tx.execute("DELETE FROM character_popularity_rating", [])?;

    let global_game_count: f64 =
        tx.query_row("SELECT COUNT(*) FROM  temp.recent_games", params![], |r| {
            r.get(0)
        })?;

    if global_game_count == 0.0 {
        info!("No new games have been recorded. Unable to calcualate character popularity");
        return Ok(());
    }

    for c in 0..CHAR_COUNT {
        let char_count: f64 = tx.query_row(
            "SELECT
                    (SELECT COUNT(*) FROM temp.recent_games
                    WHERE char_a = ?)
                    +
                    (SELECT COUNT(*) FROM temp.recent_games
                    WHERE char_b = ?)",
            params![c, c],
            |r| r.get(0),
        )?;

        tx.execute(
            "INSERT INTO character_popularity_global VALUES(?, ?)",
            params![c, char_count / global_game_count],
        )?;
    }

    for r in 0..POP_RATING_BRACKETS {
        let rating_min = if r > 0 { (900 + r * 100) as f64 } else { -99.0 };
        let rating_max = if r < POP_RATING_BRACKETS - 1 {
            (1000 + (r + 1) * 100) as f64
        } else {
            99.0
        };

        let rating_game_count: f64 = tx.query_row(
            "SELECT
                (SELECT COUNT(*) FROM temp.recent_games
                WHERE value_a >= ? AND value_a < ? AND deviation_a < ?)
                +
                (SELECT COUNT(*) FROM temp.recent_games
                WHERE value_b >= ? AND value_b < ? AND deviation_b < ?)
                ",
            params![
                rating_min,
                rating_max,
                LOW_DEVIATION,
                rating_min,
                rating_max,
                LOW_DEVIATION
            ],
            |r| r.get(0),
        )?;

        for c in 0..CHAR_COUNT {
            let char_count: f64 = tx.query_row(
                "SELECT
                    (SELECT COUNT(*) FROM temp.recent_games
                        WHERE char_a =?
                            AND value_a >= ?
                            AND value_a < ?
                            AND deviation_a < ?)
                    +
                    (SELECT COUNT(*) FROM temp.recent_games
                        WHERE char_b =?
                            AND value_b >= ?
                            AND value_b < ?
                            AND deviation_b < ?)",
                params![
                    c,
                    rating_min,
                    rating_max,
                    LOW_DEVIATION,
                    c,
                    rating_min,
                    rating_max,
                    LOW_DEVIATION
                ],
                |r| r.get(0),
            )?;

            tx.execute(
                "INSERT INTO character_popularity_rating VALUES(?, ?, ?)",
                params![c, r, 2.0 * char_count / rating_game_count.max(1.0)],
            )?;
        }
    }

    tx.execute("DROP TABLE temp.recent_games", [])?;

    tx.commit()?;
    info!(
        "Updated character popularity - {}ms",
        (Utc::now() - then).num_milliseconds()
    );
    Ok(())
}

pub fn update_rankings_once() {
    let mut conn = Connection::open(DB_NAME).unwrap();
    update_rankings(&mut conn).unwrap();
}

pub fn update_rankings(conn: &mut Connection) -> Result<()> {
    info!("Updating rankings");
    let then = Utc::now();
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM ranking_global", [])?;
    tx.execute("DELETE FROM ranking_character", [])?;

    tx.execute(
        "INSERT INTO ranking_global (global_rank, id, char_id)
         SELECT ROW_NUMBER()
         OVER (ORDER BY value DESC) as global_rank, player_ratings.id, char_id
         FROM player_ratings
            LEFT JOIN cheater_status on player_ratings.id = cheater_status.id
            LEFT JOIN hidden_status on player_ratings.id = hidden_status.id
         WHERE deviation < ? AND cheater_status IS NULL AND hidden_status IS NULL
         ORDER BY value DESC
         LIMIT 1000",
        params![LOW_DEVIATION],
    )?;

    for c in 0..CHAR_COUNT {
        tx.execute(
            "INSERT INTO ranking_character (character_rank, id, char_id)
             SELECT ROW_NUMBER() 
             OVER (ORDER BY value DESC) as character_rank, player_ratings.id, char_id
             FROM player_ratings
                LEFT JOIN cheater_status on player_ratings.id = cheater_status.id
                LEFT JOIN hidden_status on player_ratings.id = hidden_status.id
             WHERE deviation < ? AND char_id = ? AND cheater_status IS NULL AND hidden_status IS NULL
             ORDER BY value DESC
             LIMIT 1000",
            params![LOW_DEVIATION, c],
        )?;
    }

    tx.commit()?;
    info!(
        "Updated rankings - {}ms",
        (Utc::now() - then).num_milliseconds()
    );
    Ok(())
}

pub fn update_decay(conn: &mut Connection, timestamp: i64) -> Result<()> {
    info!("Updating decay");
    let then = Utc::now();

    let tx = conn.transaction()?;

    let mut players = {
        let mut players = FxHashMap::default();

        let mut stmt = tx
            .prepare(
                "SELECT
                    id, char_id, wins, losses, value, deviation, last_decay,
                    top_rating_value, top_rating_deviation, top_rating_timestamp,
                    top_defeated_id, top_defeated_char_id, top_defeated_name,
                    top_defeated_value, top_defeated_deviation, top_defeated_floor,
                    top_defeated_timestamp, 0
                FROM player_ratings 
                WHERE deviation < 350.0",
            )
            .unwrap();
        let mut rows = stmt.query([]).unwrap();

        while let Some(row) = rows.next().unwrap() {
            let player = RatedPlayer::from_row(row);
            players.insert((player.id, player.char_id), player);
        }
        players
    };

    let mut total_decay = 0;
    for p in &mut players {
        total_decay += p.1.decay(timestamp);
    }

    info!("Executed {} decay cycles.", total_decay);

    for player in players.values() {
        tx.execute(
            "UPDATE player_ratings SET
            deviation = ?, last_decay = ? WHERE 
            id = ? AND char_id = ?",
            params![
                player.rating.deviation,
                player.last_decay,
                player.id,
                player.char_id,
            ],
        )
        .unwrap();
    }

    tx.commit()?;
    info!(
        "Updated decay - {}ms",
        (Utc::now() - then).num_milliseconds()
    );
    Ok(())
}

pub async fn test_decay_matchups() {
    let mut conn = Connection::open(DB_NAME).unwrap();

    decay_matchups(&mut conn, Utc::now().timestamp()).unwrap();
}

fn decay_matchups(conn: &mut Connection, _timestamp: i64) -> Result<()> {
    conn.create_scalar_function("sqrt", 1, FunctionFlags::SQLITE_DETERMINISTIC, |ctx| {
        ctx.get::<f64>(0).map(f64::sqrt)
    })?;

    let tx = conn.transaction()?;

    //tx.execute(
    //    "UPDATE player_matchups
    //    SET rating_deviation = min(
    //             :initial_deviation,
    //             sqrt(rating_deviation * rating_deviation + :c * :c)),
    //        rating_timestamp = :timestamp
    //    WHERE
    //        rating_deviation < :initial_deviation AND
    //        rating_timestamp + :rating_period < :timestamp",
    //    named_params! {
    //        ":initial_deviation": glicko::INITIAL_DEVIATION,
    //        ":c": DECAY_CONSTANT,
    //        ":timestamp": timestamp,
    //        ":rating_period": RATING_PERIOD,
    //    },
    //)?;

    tx.execute(
        "UPDATE global_matchups
        SET rating_deviation = min(
                 :initial_deviation, 
                 sqrt(rating_deviation * rating_deviation + :c * :c))
        WHERE 
            rating_deviation < :initial_deviation",
        named_params! {
            ":initial_deviation": glicko::INITIAL_DEVIATION,
            ":c": DECAY_CONSTANT,
        },
    )?;

    tx.execute(
        "UPDATE top_1000_matchups
        SET rating_deviation = min(
                 :initial_deviation, 
                 sqrt(rating_deviation * rating_deviation + :c * :c))
        WHERE 
            rating_deviation < :initial_deviation",
        named_params! {
            ":initial_deviation": glicko::INITIAL_DEVIATION,
            ":c": DECAY_CONSTANT,
        },
    )?;
    tx.execute(
        "UPDATE top_100_matchups
        SET rating_deviation = min(
                 :initial_deviation, 
                 sqrt(rating_deviation * rating_deviation + :c * :c))
        WHERE 
            rating_deviation < :initial_deviation",
        named_params! {
            ":initial_deviation": glicko::INITIAL_DEVIATION,
            ":c": DECAY_CONSTANT,
        },
    )?;
    tx.execute(
        "UPDATE proportional_matchups
        SET rating_deviation = min(
                 :initial_deviation, 
                 sqrt(rating_deviation * rating_deviation + :c * :c))
        WHERE 
            rating_deviation < :initial_deviation",
        named_params! {
            ":initial_deviation": glicko::INITIAL_DEVIATION,
            ":c": DECAY_CONSTANT,
        },
    )?;

    tx.commit()?;

    Ok(())
}

pub fn calc_fraud_index(conn: &mut Connection) -> Result<()> {
    let then = Utc::now();
    info!("Calculating fraud index");
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM fraud_index", [])?;
    tx.execute("DELETE FROM fraud_index_higher_rated", [])?;
    tx.execute("DELETE FROM fraud_index_highest_rated", [])?;

    {
        let mut stmt = tx
            .prepare(
                "select 
                    char_id, 
                    count(*), 
                    avg(value - 
                        (avg_value - (1.0 / char_count) * value)
                        * char_count
                        / (char_count - 1.0))
            from
                (
                    select id, avg_value, char_count from
                    (
                        select 
                            id, 
                            avg(value) as avg_value, 
                            count(char_id) as char_count
                        from player_ratings
                        where deviation < ? and wins + losses >= 200
                        group by id
                    ) as averages
                    where char_count > 1
                ) as filtered_averages

                join

                (
                    select id, char_id, value
                    from player_ratings
                    where deviation < ? and wins + losses >= 200
                ) as char_ratings

                on filtered_averages.id = char_ratings.id
                
                where char_ratings.value > filtered_averages.avg_value

            group by char_id;",
            )
            .unwrap();

        let mut rows = stmt.query(params![LOW_DEVIATION, LOW_DEVIATION]).unwrap();

        while let Some(row) = rows.next().unwrap() {
            let char_id: i64 = row.get(0).unwrap();
            let player_count: i64 = row.get(1).unwrap();
            let avg_delta: f64 = row.get(2).unwrap();
            tx.execute(
                "INSERT INTO fraud_index VALUES(?, ?, ?)",
                params![char_id, player_count, avg_delta],
            )
            .unwrap();
        }

        let mut stmt = tx
            .prepare(
                "select 
                    char_id, 
                    count(*), 
                    avg(value - 
                        (avg_value - (1.0 / char_count) * value)
                        * char_count
                        / (char_count - 1.0))
            from
                (
                    select id, avg_value, char_count from
                    (
                        select 
                            id, 
                            avg(value) as avg_value, 
                            count(char_id) as char_count
                        from player_ratings
                        where deviation < ? and wins + losses >= 200
                        group by id
                    ) as averages
                    where char_count > 1
                ) as filtered_averages

                join

                (
                    select id, char_id, value
                    from player_ratings
                    where deviation < ? and wins + losses >= 200
                ) as char_ratings

                on filtered_averages.id = char_ratings.id

                where char_ratings.value > filtered_averages.avg_value
                    and char_ratings.value > 1500

            group by char_id;",
            )
            .unwrap();

        let mut rows = stmt.query(params![LOW_DEVIATION, LOW_DEVIATION]).unwrap();

        while let Some(row) = rows.next().unwrap() {
            let char_id: i64 = row.get(0).unwrap();
            let player_count: i64 = row.get(1).unwrap();
            let avg_delta: f64 = row.get(2).unwrap();
            tx.execute(
                "INSERT INTO fraud_index_higher_rated VALUES(?, ?, ?)",
                params![char_id, player_count, avg_delta],
            )
            .unwrap();
        }

        let mut stmt = tx
            .prepare(
                "select 
                    char_id, 
                    count(*), 
                    avg(value - 
                        (avg_value - (1.0 / char_count) * value)
                        * char_count
                        / (char_count - 1.0))
            from
                (
                    select id, avg_value, char_count from
                    (
                        select
                            id,
                            avg(value) as avg_value, 
                            count(char_id) as char_count
                        from player_ratings
                        where deviation < ? and wins + losses >= 200
                        group by id
                    ) as averages
                    where char_count > 1
                ) as filtered_averages

                join

                (
                    select id, char_id, value
                    from player_ratings
                    where deviation < ? and wins + losses >= 200
                ) as char_ratings

                on filtered_averages.id = char_ratings.id

                where char_ratings.value > filtered_averages.avg_value 
                    and char_ratings.value > 1800

            group by char_id;",
            )
            .unwrap();

        let mut rows = stmt.query(params![LOW_DEVIATION, LOW_DEVIATION]).unwrap();

        while let Some(row) = rows.next().unwrap() {
            let char_id: i64 = row.get(0).unwrap();
            let player_count: i64 = row.get(1).unwrap();
            let avg_delta: f64 = row.get(2).unwrap();
            tx.execute(
                "INSERT INTO fraud_index_highest_rated VALUES(?, ?, ?)",
                params![char_id, player_count, avg_delta],
            )
            .unwrap();
        }
    }

    tx.commit()?;

    info!(
        "Updated fraud index - {}ms",
        (Utc::now() - then).num_milliseconds()
    );

    Ok(())
}

#[derive(Debug)]
pub struct Game {
    timestamp: i64,
    id_a: i64,
    name_a: String,
    char_a: i64,
    platform_a: i64,
    id_b: i64,
    name_b: String,
    char_b: i64,
    platform_b: i64,
    winner: i64,
    game_floor: i64,
}

impl Game {
    pub fn from_row(row: &Row) -> Self {
        Self {
            timestamp: row.get(0).unwrap(),
            id_a: row.get(1).unwrap(),
            name_a: row.get(2).unwrap(),
            char_a: row.get(3).unwrap(),
            platform_a: row.get(4).unwrap(),
            id_b: row.get(5).unwrap(),
            name_b: row.get(6).unwrap(),
            char_b: row.get(7).unwrap(),
            platform_b: row.get(8).unwrap(),
            winner: row.get(9).unwrap(),
            game_floor: row.get(10).unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct RatedPlayer {
    pub id: i64,
    pub char_id: i64,
    pub win_count: i64,
    pub loss_count: i64,
    pub rating: Rating,
    pub last_decay: i64,

    pub top_rating: Option<TopRating>,

    pub top_defeated: Option<TopDefeated>,

    pub character_rank: Option<i64>,
}

#[derive(Debug)]
pub struct TopRating {
    value: f64,
    deviation: f64,
    timestamp: i64,
}

#[derive(Debug)]
pub struct TopDefeated {
    id: i64,
    char_id: i64,
    name: String,
    value: f64,
    deviation: f64,
    floor: i64,
    timestamp: i64,
}

impl RatedPlayer {
    fn new(id: i64, char_id: i64, timestamp: i64) -> Self {
        Self {
            id,
            char_id,
            win_count: 0,
            loss_count: 0,
            rating: Rating::default(),
            last_decay: timestamp,

            top_rating: None,
            top_defeated: None,

            character_rank: None,
        }
    }
    pub fn new_from_rating(id: i64, char_id: i64, timestamp: i64, rating: f64) -> Self {
        Self {
            id,
            char_id,
            win_count: 0,
            loss_count: 0,
            rating: Rating {
                value: rating,
                ..Rating::default()
            },
            last_decay: timestamp,
            top_rating: None,
            top_defeated: None,
            character_rank: None,
        }
    }
    pub fn from_row(row: &Row) -> Self {
        Self {
            id: row.get(0).unwrap(),
            char_id: row.get(1).unwrap(),
            win_count: row.get(2).unwrap(),
            loss_count: row.get(3).unwrap(),
            rating: Rating::new(row.get(4).unwrap(), row.get(5).unwrap()),
            last_decay: row.get(6).unwrap(),

            top_rating: row
                .get(7)
                .map(|value| {
                    Some(TopRating {
                        value,
                        deviation: row.get(8).unwrap(),
                        timestamp: row.get(9).unwrap(),
                    })
                })
                .unwrap_or_default(),

            top_defeated: row
                .get(10)
                .map(|id| {
                    Some(TopDefeated {
                        id,
                        char_id: row.get(11).unwrap(),
                        name: row.get(12).unwrap(),
                        value: row.get(13).unwrap(),
                        deviation: row.get(14).unwrap(),
                        floor: row.get(15).unwrap(),
                        timestamp: row.get(16).unwrap(),
                    })
                })
                .unwrap_or_default(),

            character_rank: row.get(17).unwrap_or(None),
        }
    }

    fn decay(&mut self, timestamp: i64) -> i64 {
        let delta = timestamp - self.last_decay;
        if delta < 0 {
            self.last_decay = timestamp;
            0
        } else if delta > RATING_PERIOD {
            self.rating
                .decay_deviation(delta / RATING_PERIOD, DECAY_CONSTANT);

            //This is actually going to round some things off but I don't really mind
            //The difference should be extremely minor in any case
            self.last_decay = timestamp;

            delta / RATING_PERIOD
        } else {
            0
        }
    }

    fn update_top_rating(&mut self, timestamp: i64) {
        if self.rating.deviation < LOW_DEVIATION
            && self
                .top_rating
                .as_ref()
                .map(|r| self.rating.value >= r.value)
                .unwrap_or(true)
        {
            self.top_rating = Some(TopRating {
                value: self.rating.value,
                deviation: self.rating.deviation,
                timestamp,
            });
        }
    }

    fn update_top_defeated(
        &mut self,
        id: i64,
        char_id: i64,
        name: String,
        opponent_rating: Rating,
        floor: i64,
        timestamp: i64,
    ) {
        if opponent_rating.deviation < LOW_DEVIATION
            && self
                .top_defeated
                .as_ref()
                .map(|t| opponent_rating.value > t.value)
                .unwrap_or(true)
        {
            self.top_defeated = Some(TopDefeated {
                id,
                char_id,
                name,
                value: opponent_rating.value,
                deviation: opponent_rating.deviation,
                floor,
                timestamp,
            });
        }
    }
}
