#![forbid(unsafe_code)]
//! Thin binary: Premier League registered-squad + FM26 Monte Carlo snapshot driver.

fn main() {
    footy_matchforecast::driver::run_premier_league();
}
