use crate::{api, rater};
use chrono::Utc;
use rocket::{
    fs::NamedFile,
    http::{hyper::header::CACHE_CONTROL, Header},
    response::{self, Builder, Redirect, Responder},
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
    ("JA", "Jack-O'"),
    ("HA", "Happy Chaos"),
];

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
                player,
                api::stats,
                api::top_all,
                api::top_char
            ],
        )
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

#[get("/top/all")]
async fn top_all(conn: RatingsDbConn) -> Cached<Template> {
    #[derive(Serialize)]
    struct Context {
        stats: api::Stats,
        players: Vec<api::RankingPlayer>,
    }

    let context = Context {
        stats: api::stats_inner(&conn).await,
        players: api::top_all_inner(&conn).await,
    };

    let delta = context.stats.last_update + rater::RATING_PERIOD - Utc::now().timestamp();
    Cached::new(Template::render("top_100", &context), delta)
}

#[get("/top/<character_short>")]
async fn top_char(conn: RatingsDbConn, character_short: &str) -> Option<Cached<Template>> {
    #[derive(Serialize)]
    struct Context {
        stats: api::Stats,
        players: Vec<api::RankingPlayer>,
        character: &'static str,
        character_short: &'static str,
        all_characters: &'static [(&'static str, &'static str)],
    }

    if let Some(char_code) = CHAR_NAMES.iter().position(|(c, _)| *c == character_short) {
        let (character_short, character) = CHAR_NAMES[char_code];
        let context = Context {
            stats: api::stats_inner(&conn).await,
            players: api::top_char_inner(&conn, char_code as i64).await,
            character,
            character_short,
            all_characters: CHAR_NAMES,
        };

        Some(Cached::new(
            Template::render("top_100_char", &context),
            context.stats.last_update + rater::RATING_PERIOD - Utc::now().timestamp(),
        ))
    } else {
        None
    }
}

#[get("/player/<player_id>")]
async fn player(conn: RatingsDbConn, player_id: &str) -> Option<Cached<Template>> {
    let id = i64::from_str_radix(player_id, 16).unwrap();

    #[derive(Serialize)]
    struct Context {
        stats: api::Stats,
        player: api::PlayerData,
    }

    let stats = api::stats_inner(&conn).await;

    if let Some(player) = api::get_player_data(&conn, id).await {
        let context = Context { stats, player };
        Some(Cached::new(
            Template::render("player", &context),
            context.stats.last_update + rater::RATING_PERIOD - Utc::now().timestamp(),
        ))
    } else {
        None
    }
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