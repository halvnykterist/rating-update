use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Response<T> {
    headers: ResponseHeader,
    pub body: T,
}

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

#[derive(Deserialize, Debug)]
pub struct Replays {
    int1: i64,
    int2: i64,
    int3: i64,
    pub replays: Vec<Replay>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Replay {
    int1: u64,
    int2: i64,
    pub floor: i64,
    pub player1_character: i64,
    pub player2_character: i64,
    pub player1: Player,
    pub player2: Player,
    pub winner: i64,
    pub timestamp: String,
    int7: i64,
    views: u64,
    int8: i64,
    likes: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Player {
    pub id: String,
    pub name: String,
    string1: String,
    string2: String,
    pub platform: i64,
    int1: i64,
}
