use criterion::{criterion_group, criterion_main, Criterion};

fn load_player(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let http_client = reqwest::Client::new();

    runtime.block_on(async { tokio::spawn(rating_update::website::run()); });

    c.bench_function("load_player", |b| {

        b.to_async(&runtime).iter(|| async {
            let response = http_client.get("http://localhost/player/2EC4D20AC79D536/KY").send().await.unwrap();

            assert_eq!(response.status(), reqwest::StatusCode::OK);
            response.bytes().await.unwrap();
        });
    });
}

criterion_group!(bench, load_player);
criterion_main!(bench);
