use std::path::{Path, PathBuf};

use rocket::{
    fs::NamedFile,
    serde::{json::Json, Serialize},
};
use rocket_sync_db_pools::database;
use rusqlite::Connection;

pub async fn run() {
    rocket::build()
        .attach(RatingsDbConn::fairing())
        .mount("/", routes![index, files, do_test])
        .ignite()
        .await
        .unwrap()
        .launch()
        .await
        .unwrap();
}

#[database("ratings")]
struct RatingsDbConn(Connection);

#[get("/")]
async fn index() -> Option<NamedFile> {
    NamedFile::open(Path::new("static/index.html")).await.ok()
}

#[get("/<file..>")]
async fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static/").join(file)).await.ok()
}

#[get("/api/test")]
async fn do_test(conn: RatingsDbConn) -> Json<Vec<Flah>> {
    let res = conn
        .run(|c| {
            let mut statement = c.prepare("SELECT foo, bar FROM test").unwrap();
            statement
                .query_map([], |row| {
                    let (foo, bar): (String, i32) = row.try_into().unwrap();
                    Ok(Flah { foo, bar} )
                })
                .unwrap()
                .into_iter()
                .map(|r| r.unwrap())
                .collect()
        })
        .await;

    Json(res)
}

#[derive(Serialize)]
struct Flah {
    foo: String,
    bar: i32,
}
