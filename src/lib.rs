#[macro_use]
extern crate rocket;
#[macro_use]
extern crate log;

mod api;
mod glicko;
mod ggst_api;
mod responses;
mod requests;
pub mod rater;
pub mod website;
