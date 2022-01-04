use chrono::{DateTime, NaiveDate, Utc};
use glob::glob;
use lazy_static::lazy_static;
use rocket::serde::json::serde_json;
use rusqlite::{params, Connection, Transaction};
use serde::Deserialize;
use std::{error::Error, fs::File, io::BufReader, sync::Mutex, time::Duration};
use tokio::time;

const SYS_CONSTANT: f64 = 0.01;
const DB_NAME: &str = "ratings.sqlite";

lazy_static! {
    pub static ref RUNTIME_DATA: Mutex<RuntimeData> = Mutex::new(RuntimeData {});
}

pub struct RuntimeData {}

pub fn init_database() -> Result<(), Box<dyn Error>> {
    //check if DB exists first, don't do this if so
    let conn = Connection::open(DB_NAME)?;

    conn.execute_batch(include_str!("../init.sql"))?;

    Ok(())
}

pub fn load_json_data(path: &str) -> Result<(), Box<dyn Error>> {
    let mut conn = Connection::open(DB_NAME)?;

    #[derive(Deserialize)]
    #[allow(non_snake_case)]
    struct RawGame {
        time: String,
        floor: u32,
        winner: u32,
        playerAID: String,
        playerBID: String,
        playerAName: String,
        playerBName: String,
        playerACharCode: usize,
        playerBCharCode: usize,
    }

    for entry in glob(&format!("{}*.json", path)).unwrap() {
        let tx = conn.transaction().unwrap();
        match entry {
            Ok(path) => {
                info!("Loading replays from: {:?}", path);
                let file = File::open(path).unwrap();
                let reader = BufReader::new(file);
                let games: Vec<RawGame> = serde_json::from_reader(reader).unwrap();
                for g in games {
                    if g.time != "" {
                        let mut dt = g.time.split(' ');
                        let mut date = dt.next().unwrap().split('-');
                        let mut time = dt.next().unwrap().split(':');
                        let timestamp = NaiveDate::from_ymd(
                            date.next().unwrap().parse().unwrap(),
                            date.next().unwrap().parse().unwrap(),
                            date.next().unwrap().parse().unwrap(),
                        )
                        .and_hms(
                            time.next().unwrap().parse().unwrap(),
                            time.next().unwrap().parse().unwrap(),
                            time.next().unwrap().parse().unwrap(),
                        );
                        let timestamp = DateTime::<Utc>::from_utc(timestamp, Utc);
                        add_game(
                            &tx,
                            ggst_api::Match {
                                timestamp,
                                floor: ggst_api::Floor::from_u8(g.floor as u8).unwrap(),
                                winner: match g.winner {
                                    1 => ggst_api::Winner::Player1,
                                    2 => ggst_api::Winner::Player2,
                                    _ => panic!("Bad winner"),
                                },
                                players: (
                                    ggst_api::Player {
                                        id: g.playerAID.parse().unwrap(),
                                        character: ggst_api::Character::from_u8(
                                            g.playerACharCode as u8,
                                        )
                                        .unwrap(),
                                        name: g.playerAName.clone(),
                                    },
                                    ggst_api::Player {
                                        id: g.playerBID.parse().unwrap(),
                                        character: ggst_api::Character::from_u8(
                                            g.playerBCharCode as u8,
                                        )
                                        .unwrap(),
                                        name: g.playerBName.clone(),
                                    },
                                ),
                            },
                        )
                    }
                }
            }
            Err(e) => error!("Couldn't read path: {:?}", e),
        }
        tx.commit().unwrap();
    }

    Ok(())
}

pub async fn run() {
    let mut conn = Connection::open(DB_NAME).unwrap();

    grab_games(&mut conn, 100).await;

    let mut interval = time::interval(Duration::from_secs(60));
    loop {
        interval.tick().await;
        grab_games(&mut conn, 5).await;
    }
}

async fn grab_games(conn: &mut Connection, pages: usize) {
    let replays = ggst_api::get_replays(
        &ggst_api::Context::default(),
        pages,
        127,
        ggst_api::Floor::F1,
        ggst_api::Floor::Celestial,
    )
    .await
    .unwrap();

    let (replays, errors): (Vec<_>, Vec<_>) = (replays.0.collect(), replays.1.collect());

    let tx = conn.transaction().unwrap();
    for r in &replays {
        add_game(&tx, r.clone());
    }

    tx.commit().unwrap();

    let count: i64 = conn
        .query_row("select count(*) from games", [], |r| r.get(0))
        .unwrap();

    info!(
        "Grabbed {} games and {} errors. New game count: {}",
        replays.len(),
        errors.len(),
        count
    );
}

fn add_game(conn: &Transaction, game: ggst_api::Match) {
    let ggst_api::Match {
        timestamp,
        players: (a, b),
        floor: game_floor,
        winner,
    } = game;
    update_player(conn, a.id, &a.name);
    update_player(conn, b.id, &b.name);

    conn.execute(
        "INSERT OR IGNORE INTO games (
            timestamp, 
            id_a, 
            name_a,
            char_a,
            id_b,
            name_b,
            char_b,
            winner, 
            game_floor
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            timestamp.timestamp(),
            a.id,
            a.name,
            a.character.to_u8(),
            b.id,
            b.name,
            b.character.to_u8(),
            match winner {
                ggst_api::Winner::Player1 => 1,
                ggst_api::Winner::Player2 => 2,
            },
            game_floor.to_u8(),
        ],
    )
    .unwrap();
}

fn update_player(conn: &Transaction, id: i64, name: &str) {
    conn.execute(
        "REPLACE INTO players(id, name) VALUES(?, ?)",
        params![id, name],
    )
    .unwrap();
}

fn update_ratings(conn: &Connection) {
    //figure out what our last timestamp was
    //grab all the games from that last timestamp to the next rating period
    //
    //for each game:
    //    either grab or make the player_ratings
    //
}
