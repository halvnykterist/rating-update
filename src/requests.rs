use serde_derive::{Deserialize, Serialize};

const VERSION: &str = "0.1.8";

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

pub fn generate_replay_request(
    index: usize,
    replays_per_page: usize,
    token: &str,
) -> Request<ReplayRequest> {
    Request {
        header: RequestHeader {
            player_id: "230129212655563979".to_owned(),
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
