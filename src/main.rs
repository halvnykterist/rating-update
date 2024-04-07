#![feature(proc_macro_hygiene, decl_macro)]

use simplelog::*;
use std::{fs::File, ops::Deref};
use tokio::try_join;
use dotenv::dotenv;

use rating_update::{rater, website};

fn init_logging() {
    if cfg!(debug_assertions) {
        CombinedLogger::init(vec![
            TermLogger::new(LevelFilter::Debug, Config::default(), TerminalMode::Mixed),
            WriteLogger::new(
                LevelFilter::Info,
                Config::default(),
                File::create("output.log").unwrap(),
            ),
        ])
        .unwrap();
    } else {
        CombinedLogger::init(vec![
            TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed),
            WriteLogger::new(
                LevelFilter::Info,
                Config::default(),
                File::create("output.log").unwrap(),
            ),
        ])
        .unwrap();
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    dotenv().ok();
    init_logging();

    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.get(0).map(|r| r.deref()) {
        Some("init") => {
            rater::init_database().unwrap();
        }
        Some("reset") => {
            rater::reset_database().unwrap();
        }
        Some("update") => {
            rater::update_once().await;
        }
        Some("rankings") => {
            rater::update_rankings_once();
        }
        Some("fraud") => {
            rater::update_fraud_once().await;
        }
        Some("mark_cheater") => {
            rater::mark_cheater(
                args.get(1).map(|r| r.deref()),
                args.get(2).map(|r| r.deref()),
                args.get(3).map(|r| r.deref()),
            )
            .await;
        }
        Some("mark_vip") => {
            rater::mark_vip(args.get(1).unwrap(), args.get(2).unwrap());
        }
        Some("mark_hidden") => {
            rater::mark_hidden(args.get(1).unwrap(), args.get(2).unwrap());
        }
        Some("print_rankings") => {
            rater::print_rankings();
        }
        Some("decay") => {
            rater::update_decay_once().await;
        }
        Some("decay_matchups") => {
            rater::test_decay_matchups().await;
        }
        Some("names") => {
            rater::reset_names().unwrap();
        }
        Some("distribution") => {
            rater::reset_distribution().unwrap();
        }
        Some("pull") => {
            rater::pull().await;
        }
        Some("nothoughts") => {
            website::run().await;
        }
        Some(x) => {
            println!("Unrecognized argument: {}", x);
        }
        None => {
            if let Err(err) = try_join!(
                async {
                    tokio::spawn(website::run()).await?;
                    Ok(())
                },
                rater::run()
            ) {
                eprintln!("{:?}", err);
                std::process::exit(1);
            }
        }
    }
}
