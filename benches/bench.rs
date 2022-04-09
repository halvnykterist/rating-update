use criterion::{criterion_group, criterion_main, Criterion};
use futures::prelude::*;

fn load_player(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let http_client = reqwest::Client::new();

    runtime.block_on(async {
        tokio::spawn(rating_update::website::run());
    });

    c.bench_function("load_player", |b| {
        // Find a number of players to run the benchmark on
        let players = {
            let db_connection = rusqlite::Connection::open(rating_update::rater::DB_NAME).unwrap();
            let mut stmt = db_connection
                .prepare("SELECT id, char_id FROM ranking_global LIMIT 50")
                .unwrap();

            let mut rows = stmt.query([]).unwrap();
            let mut players = Vec::<(i64, usize)>::new();
            while let Some(row) = rows.next().unwrap() {
                players.push((row.get(0).unwrap(), row.get(1).unwrap()));
            }

            assert_eq!(players.len(), 50);
            players
        };

        b.to_async(&runtime).iter(|| async {
            let http_client = &http_client;
            futures::stream::iter(&players)
                .map(|&(id, char_id)| async move {
                    let response = http_client
                        .get(format!(
                            "http://localhost/player/{:X}/{}",
                            id,
                            rating_update::website::CHAR_NAMES[char_id].0
                        ))
                        .send()
                        .await
                        .unwrap();

                    assert_eq!(response.status(), reqwest::StatusCode::OK);
                    response.bytes().await.unwrap();
                })
                .buffer_unordered(10)
                .for_each(|()| async {})
                .await;
        });
    });
}

criterion_group!(bench, load_player);
criterion_main!(bench);
