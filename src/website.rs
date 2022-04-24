use crate::api;
use rocket::{
    fs::NamedFile,
    http::{hyper::header::CACHE_CONTROL, Header},
    response::{self, Redirect, Responder},
    serde::Serialize,
    Request,
};
use rocket_dyn_templates::Template;
use rocket_sync_db_pools::database;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub const CHAR_NAMES: &[(&str, &str)] = &[
    ("SO", "Sol"),
    ("KY", "Ky"),
    ("MA", "May"),
    ("AX", "Axl"),
    ("CH", "Chipp"),
    ("PO", "Potemkin"),
    ("FA", "Faust"),
    ("MI", "Millia"),
    ("ZA", "Zato-1"),
    ("RA", "Ramlethal"),
    ("LE", "Leo"),
    ("NA", "Nagoriyuki"),
    ("GI", "Giovanna"),
    ("AN", "Anji"),
    ("IN", "I-No"),
    ("GO", "Goldlewis"),
    ("JC", "Jack-O'"),
    ("HA", "Happy Chaos"),
    ("BA", "Baiken"),
    ("TE", "Testament"),
];

pub const PLATFORM: &str = &"PC";

pub async fn run() {
    rocket::build()
        .attach(RatingsDbConn::fairing())
        .attach(Template::fairing())
        .mount(
            "/",
            routes![
                index,
                files,
                top_all,
                top_char,
                matchups,
                character_popularity,
                player_distr_forward,
                player_distribution,
                player,
                player_char,
                player_char_history,
                search,
                about,
                stats,
                supporters,
                api::stats,
                api::player_rating,
                api::player_rating_accuracy,
                api::top_all,
                api::top_char,
                api::search,
                api::search_exact,
                api::outcomes,
                api::floor_rating_distribution,
                api::rating_experience,
                api::rating_experience_player,
            ],
        )
        .register("/", catchers![catch_404, catch_500, catch_503])
        .ignite()
        .await
        .unwrap()
        .launch()
        .await
        .unwrap();
}

#[database("ratings")]
pub struct RatingsDbConn(Connection);

#[get("/")]
async fn index() -> Redirect {
    Redirect::to(uri!(top_all))
}

#[get("/about")]
async fn about(conn: RatingsDbConn) -> Cached<Template> {
    api::add_hit(&conn, format!("about")).await;

    #[derive(Serialize)]
    struct Context {
        platform: &'static str,
        is_pc: bool,
        all_characters: &'static [(&'static str, &'static str)],
    }
    let context = Context {
        platform: PLATFORM,
        is_pc: PLATFORM == "PC",
        all_characters: CHAR_NAMES,
    };

    Cached::new(Template::render("about", &context), 999)
}

#[get("/stats")]
async fn stats(conn: RatingsDbConn) -> Cached<Template> {
    api::add_hit(&conn, format!("stats")).await;

    #[derive(Serialize)]
    struct Context {
        platform: &'static str,
        is_pc: bool,
        stats: api::Stats,
        all_characters: &'static [(&'static str, &'static str)],
    }
    let context = Context {
        platform: PLATFORM,
        is_pc: PLATFORM == "PC",
        stats: api::stats_inner(&conn).await,
        all_characters: CHAR_NAMES,
    };

    Cached::new(Template::render("stats", &context), 999)
}

#[get("/supporters")]
async fn supporters(conn: RatingsDbConn) -> Cached<Template> {
    api::add_hit(&conn, format!("supporters")).await;
    #[derive(Serialize)]
    struct Context {
        platform: &'static str,
        is_pc: bool,
        players: Vec<api::VipPlayer>,
        all_characters: &'static [(&'static str, &'static str)],
    }

    Cached::new(
        Template::render(
            "supporters",
            &Context {
                platform: PLATFORM,
                is_pc: PLATFORM == "PC",
                players: api::get_supporters(&conn).await,
                all_characters: CHAR_NAMES,
            },
        ),
        999,
    )
}

#[get("/top/all")]
async fn top_all(conn: RatingsDbConn) -> Cached<Template> {
    api::add_hit(&conn, format!("top/all")).await;

    #[derive(Serialize)]
    struct Context {
        platform: &'static str,
        is_pc: bool,
        players: Vec<api::RankingPlayer>,
        all_characters: &'static [(&'static str, &'static str)],
    }

    let players = api::top_all_inner(&conn).await;
    let context = Context {
        platform: PLATFORM,
        is_pc: PLATFORM == "PC",
        players,
        all_characters: CHAR_NAMES,
    };

    Cached::new(Template::render("top_100", &context), 999)
}

#[get("/top/<character_short>")]
async fn top_char(conn: RatingsDbConn, character_short: &str) -> Option<Cached<Template>> {
    api::add_hit(&conn, format!("top/{}", character_short)).await;

    #[derive(Serialize)]
    struct Context {
        platform: &'static str,
        is_pc: bool,
        players: Vec<api::RankingPlayer>,
        character: &'static str,
        character_short: &'static str,
        all_characters: &'static [(&'static str, &'static str)],
    }

    if let Some(char_code) = CHAR_NAMES.iter().position(|(c, _)| *c == character_short) {
        let (character_short, character) = CHAR_NAMES[char_code];

        let players = api::top_char_inner(&conn, char_code as i64).await;
        let context = Context {
            platform: PLATFORM,
            is_pc: PLATFORM == "PC",
            players,
            character,
            character_short,
            all_characters: CHAR_NAMES,
        };

        Some(Cached::new(Template::render("top_100_char", &context), 999))
    } else {
        None
    }
}

#[get("/matchups")]
async fn matchups(conn: RatingsDbConn) -> Cached<Template> {
    api::add_hit(&conn, format!("matchups")).await;

    #[derive(Serialize)]
    struct Context {
        platform: &'static str,
        is_pc: bool,
        character_shortnames: Vec<&'static str>,
        matchups_global: Vec<api::CharacterMatchups>,
        matchups_high_rated: Vec<api::CharacterMatchups>,
        matchups_versus: Vec<api::VersusCharacterMatchups>,
        all_characters: &'static [(&'static str, &'static str)],
    }

    let (matchups_global, matchups_high_rated, matchups_versus) = tokio::join!(
        api::matchups_global_inner(&conn),
        api::matchups_high_rated_inner(&conn),
        api::matchups_versus(&conn),
    );

    let context = Context {
        platform: PLATFORM,
        is_pc: PLATFORM == "PC",
        character_shortnames: CHAR_NAMES.iter().map(|c| c.0).collect(),
        matchups_global,
        matchups_high_rated,
        matchups_versus,
        all_characters: CHAR_NAMES,
    };

    Cached::new(Template::render("matchups", &context), 999)
}

#[get("/character_popularity")]
async fn character_popularity(conn: RatingsDbConn) -> Cached<Template> {
    api::add_hit(&conn, format!("character_popularity")).await;

    #[derive(Serialize)]
    struct Context {
        platform: &'static str,
        is_pc: bool,
        character_shortnames: Vec<&'static str>,
        global_character_popularity: Vec<f64>,
        rank_character_popularity: Vec<api::RankCharacterPopularities>,
        fraud_stats: Vec<api::FraudStats>,
        fraud_stats_higher_rated: Vec<api::FraudStats>,
        fraud_stats_highest_rated: Vec<api::FraudStats>,
        all_characters: &'static [(&'static str, &'static str)],
    }

    let (
        (global_character_popularity, rank_character_popularity),
        fraud_stats,
        fraud_stats_higher_rated,
        fraud_stats_highest_rated,
    ) = tokio::join!(
        api::character_popularity(&conn),
        api::get_fraud(&conn),
        api::get_fraud_higher_rated(&conn),
        api::get_fraud_highest_rated(&conn),
    );

    let context = Context {
        platform: PLATFORM,
        is_pc: PLATFORM == "PC",
        character_shortnames: CHAR_NAMES.iter().map(|c| c.0).collect(),
        global_character_popularity,
        rank_character_popularity,
        fraud_stats,
        fraud_stats_higher_rated,
        fraud_stats_highest_rated,
        all_characters: CHAR_NAMES,
    };

    Cached::new(Template::render("character_popularity", &context), 999)
}

#[get("/player-distribution")]
async fn player_distr_forward() -> Redirect {
    Redirect::to(uri!("player_distribution"))
}

#[get("/player_distribution")]
async fn player_distribution(conn: RatingsDbConn) -> Cached<Template> {
    api::add_hit(&conn, format!("player_distribution")).await;

    #[derive(Serialize)]
    struct Context {
        platform: &'static str,
        is_pc: bool,
        floors: Vec<api::FloorPlayers>,
        ratings: Vec<api::RatingPlayers>,
        all_characters: &'static [(&'static str, &'static str)],
    }

    let (floors, ratings) = tokio::join!(
        api::player_floors_distribution(&conn),
        api::player_ratings_distribution(&conn),
    );
    let context = Context {
        platform: PLATFORM,
        is_pc: PLATFORM == "PC",
        floors,
        ratings,
        all_characters: CHAR_NAMES,
    };

    Cached::new(Template::render("player_distribution", &context), 999)
}

#[get("/player/<player_id>")]
async fn player(conn: RatingsDbConn, player_id: &str) -> Option<Redirect> {
    api::add_hit(&conn, format!("player/{}", player_id)).await;

    let id = i64::from_str_radix(player_id, 16).unwrap();

    if let Some(char_id) = api::get_player_highest_rated_character(&conn, id).await {
        let char_short = CHAR_NAMES[char_id as usize].0;
        Some(Redirect::to(uri!(player_char(
            player_id = player_id,
            char_id = char_short,
        ))))
    } else {
        None
    }
}
#[get("/player/<player_id>/<char_id>/history?<game_count>&<group_games>")]
async fn player_char_history(
    conn: RatingsDbConn,
    player_id: &str,
    char_id: &str,
    game_count: Option<i64>,
    group_games: Option<bool>,
) -> Option<Cached<Template>> {
    api::add_hit(&conn, format!("player/{}/{}/history", player_id, char_id)).await;

    let id = i64::from_str_radix(player_id, 16).unwrap();
    let char_id = CHAR_NAMES.iter().position(|(c, _)| *c == char_id)? as i64;
    let game_count = game_count.unwrap_or(200);
    let group_games = group_games.unwrap_or(true);

    if let Some(history) =
        api::get_player_char_history(&conn, id, char_id, game_count, group_games).await
    {
        Some(Cached::new(
            Template::render("player_char_history", &history),
            60,
        ))
    } else {
        None
    }
}

#[get("/player/<player_id>/<char_id>")]
async fn player_char(
    conn: RatingsDbConn,
    player_id: &str,
    char_id: &str,
) -> Option<Cached<Template>> {
    api::add_hit(&conn, format!("player/{}/{}", player_id, char_id)).await;

    let id = i64::from_str_radix(player_id, 16).unwrap();

    let char_id_i64 = CHAR_NAMES.iter().position(|(c, _)| *c == char_id)? as i64;

    #[derive(Serialize)]
    struct Context {
        platform: &'static str,
        is_pc: bool,
        player_id: String,
        char_id: String,
        player: api::PlayerDataChar,
        all_characters: &'static [(&'static str, &'static str)],
    }

    if let Some(player) = api::get_player_data_char(&conn, id, char_id_i64).await {
        let context = Context {
            platform: PLATFORM,
            is_pc: PLATFORM == "PC",
            player_id: player_id.to_owned(),
            char_id: char_id.to_owned(),
            player,
            all_characters: CHAR_NAMES,
        };
        Some(Cached::new(Template::render("player_char", &context), 999))
    } else {
        None
    }
}

#[get("/?<name>")]
async fn search(conn: RatingsDbConn, name: String) -> Template {
    api::add_hit(&conn, format!("search/{}", name)).await;
    #[derive(Serialize)]
    struct Context {
        platform: &'static str,
        is_pc: bool,
        search_string: String,
        players: Vec<api::SearchResultPlayer>,
        all_characters: &'static [(&'static str, &'static str)],
    }

    let players = api::search_inner(&conn, name.clone(), false).await;

    Template::render(
        "search_results",
        &Context {
            platform: PLATFORM,
            is_pc: PLATFORM == "PC",
            players,
            search_string: name,
            all_characters: CHAR_NAMES,
        },
    )
}

#[catch(404)]
async fn catch_404() -> NamedFile {
    NamedFile::open(Path::new("static/404.html")).await.unwrap()
}

#[catch(500)]
async fn catch_500() -> NamedFile {
    NamedFile::open(Path::new("static/500.html")).await.unwrap()
}

#[catch(503)]
async fn catch_503() -> NamedFile {
    NamedFile::open(Path::new("static/503.html")).await.unwrap()
}

#[get("/<file..>")]
async fn files(file: PathBuf) -> Cached<Option<NamedFile>> {
    Cached::new(
        NamedFile::open(Path::new("static/").join(file)).await.ok(),
        600,
    )
}

struct Cached<R> {
    inner: R,
    cache_control: i64,
}

impl<R> Cached<R> {
    fn new(inner: R, cache_control: i64) -> Self {
        Self {
            inner,
            cache_control,
        }
    }
}

impl<'r, 'o: 'r, R: Responder<'r, 'o>> Responder<'r, 'o> for Cached<R> {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'o> {
        self.inner.respond_to(req).map(|mut r| {
            r.adjoin_header(Header::new(
                CACHE_CONTROL.as_str(),
                format!("max-age={}", self.cache_control),
            ));
            r.adjoin_header(Header::new("age", "0"));
            r
        })
    }
}
