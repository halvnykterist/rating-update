use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Response<T> {
    pub header: ResponseHeader,
    pub body: T,
}

#[derive(Deserialize, Debug)]
pub struct ResponseHeader {
    pub token: String,
    _int1: i64,
    _date: String,
    _version1: String,
    _version2: String,
    _version3: String,
    _string1: String,
    _string2: String,
}

#[derive(Deserialize, Debug)]
pub struct Login {
    _int1: i64,
    pub data: InnerLogin,
}

#[derive(Deserialize, Debug)]
pub struct InnerLogin {
    _string1: String,
    pub name: String,
    _steam_id: String,
    _strive_id: String,
    _platform: i64,
}

#[derive(Deserialize, Debug)]
pub struct Replays {
    _int1: i64,
    _int2: i64,
    _int3: i64,
    pub replays: Vec<Replay>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Replay {
    _int1: u64,
    _int2: i64,
    pub floor: i64,
    pub player1_character: i64,
    pub player2_character: i64,
    pub player1: Player,
    pub player2: Player,
    pub winner: i64,
    pub timestamp: String,
    _int7: i64,
    _views: u64,
    _int8: i64,
    _likes: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Player {
    pub id: String,
    pub name: String,
    _string1: String,
    _string2: String,
    pub platform: i64,
    _int1: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PlayerStats {
    _int1: i64,
    pub json: String,
    _int2: i64,
}
