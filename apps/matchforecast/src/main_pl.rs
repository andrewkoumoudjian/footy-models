#![forbid(unsafe_code)]
//! Premier League match engine driver, sharing the production input adapter.
#[path = "main_mls.rs"]
mod shared;
fn main() { shared::run_premier_league(); }
