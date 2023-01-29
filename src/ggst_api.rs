use std::error::Error;
use serde_derive::Deserialize;

pub async fn get_replays() -> Vec<Replay> {
    todo!()
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct Response<T> {
    headers: ResponseHeader,
    pub body: T,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct ResponseHeader {
    id: String,
    int1: i64,
    date: String,
    version1: String,
    version2: String,
    version3: String,
    string1: String,
    string2: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct Replays {
    int1: i64,
    int2: i64,
    int3: i64,
    replays: Vec<Replay>,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct Replay {
    pub int1: u64,
    pub int2: i64,
    pub floor: i64,
    pub player1_character: i64,
    pub player2_character: i64,
    pub player1: Player,
    pub player2: Player,
    pub winner: i64,
    pub timestamp: String,
    pub int7: i64,
    pub views: u64,
    pub int8: i64,
    pub likes: u64,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug, Clone)]
pub struct Player {
    pub id: String,
    pub name: String,
    pub string1: String,
    pub string2: String,
    pub platform: i64,
    pub int1: i64,
}
