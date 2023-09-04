use serde_derive::{Deserialize, Serialize};
use std::sync::Arc;
use steamworks::{Client, TicketForWebApiResponse};
use tokio::sync::Mutex;

const VERSION: &str = "0.2.3";

#[derive(Debug, Serialize, Deserialize)]
pub struct Request<T> {
    header: RequestHeader,
    body: T,
}

#[derive(Debug, Serialize, Deserialize)]
struct RequestHeader {
    player_id: String,
    token: String,
    int1: i64,
    version: String,
    platform: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct ReplayRequest {
    int1: i64,
    index: usize,
    replays_per_page: usize,
    query: ReplayQuery,
    platforms: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
struct ReplayQuery {
    int1: i64,
    player_search: i64,
    min_floor: i64,
    max_floor: i64,
    seq1: Vec<()>,
    char_1: i64,
    char_2: i64,
    winner: i64,
    prioritize_best_bout: i64,
    int2: i64,
}

#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct PlayerStatsRequest {
    player_id: String,
    int1: i64,
    int2: i64,
    int3: i64,
    int4: i64,
    int5: i64,
}

pub fn generate_player_stats_request(player_id: String) -> Request<PlayerStatsRequest> {
    Request {
        header: RequestHeader {
            player_id: "210611080230642425".to_owned(),
            token: std::fs::read_to_string("token.txt").unwrap(),
            int1: 2,
            version: VERSION.to_owned(),
            platform: 3, //PC
        },
        body: PlayerStatsRequest {
            player_id: player_id,
            int1: 7,
            int2: -1,
            int3: 1,
            int4: -1,
            int5: -1,
        },
    }
}

pub fn generate_replay_request(
    index: usize,
    replays_per_page: usize,
    token: &str,
) -> Request<ReplayRequest> {
    Request {
        header: RequestHeader {
            player_id: "210611080230642425".to_owned(),
            token: token.to_owned(),
            int1: 2,
            version: VERSION.to_owned(),
            platform: 3, //PC
        },
        body: ReplayRequest {
            int1: 1,
            index,
            replays_per_page,
            query: ReplayQuery {
                int1: -1,
                player_search: 0,
                min_floor: 1,
                max_floor: 99,
                seq1: vec![],
                char_1: -1,
                char_2: -1,
                winner: 0,
                prioritize_best_bout: 0,
                int2: 1,
            },
            platforms: 6, //All
        },
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    int1: i64,
    steam_id: String,
    steam_hex: String,
    int2: i64,
    steam_token: String,
}

pub async fn generate_login_request() -> Request<LoginRequest> {
    let (client, single) = Client::init().unwrap();
    let user = client.user();

    let token = Arc::new(Mutex::new(Option::None));
    {
        let token = token.clone();

        let _cb = client.register_callback(move |v: TicketForWebApiResponse| {
            //println!("Got webapi auth response: {:?}", v)
            let hex: String = v
                .ticket
                .iter()
                .map(|b| format!("{:02X}", b).to_string())
                .collect::<Vec<String>>()
                .join("");
            info!("Login steam token for strive {}", hex);
            *token.try_lock().unwrap() = Some(hex);
        });
    };

    user.authentication_session_ticket_for_webapi("ggst-game.guiltygear.com");

    for _ in 0..50 {
        single.run_callbacks();
        std::thread::sleep(::std::time::Duration::from_millis(100));

        let steam_token = token.try_lock().unwrap();

        if steam_token.is_some() {
            let steam_token = steam_token.clone().unwrap();
            return Request {
                header: RequestHeader {
                    player_id: "".to_owned(),
                    token: "".to_owned(),
                    int1: 2,
                    version: VERSION.to_owned(),
                    platform: 3,
                },
                body: LoginRequest {
                    int1: 1,
                    steam_id: "76561199474089169".to_owned(),
                    steam_hex: "11000015a3b1cd1".to_owned(),
                    int2: 256,
                    steam_token,
                },
            };
        }
    }

    panic!("Timed out");
}
