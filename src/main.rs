#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate log;
use simplelog::*;
use std::{ops::Deref, fs::File};
use tokio::join;

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

#[rocket::main]
async fn main() {
    init_logging();

    let args = std::env::args().skip(1).collect::<Vec<_>>();
    match args.get(0).map(|r| r.deref()) {
        Some("init") => {
            rater::init_database().unwrap();
        }
        Some("preload") => {
            rater::load_json_data(args.get(1).unwrap()).unwrap();
        }
        Some(x) => {
            println!("Unrecognized argument: {}", x);
        }
        None => {
            join!(website::run(), rater::run());
        }
    }
}
