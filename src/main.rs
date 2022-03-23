#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate log;
use simplelog::*;
use std::{fs::File, ops::Deref};
use tokio::try_join;

mod api;
mod rater;
mod website;

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

#[rocket::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
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
        Some("names") => {
            rater::reset_names().unwrap();
        }
        Some("distribution") => {
            rater::reset_distribution().unwrap();
        }
        Some("preload") => {
            rater::load_json_data(args.get(1).unwrap()).unwrap();
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
            try_join!(tokio::spawn(website::run()), tokio::spawn(rater::run())).unwrap();
        }
    }
}
