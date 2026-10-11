#![forbid(unsafe_code)]
//! Thin binary: MLS fixture forecasts (CF Montréal + upcoming opponents).

fn main() {
    footy_matchforecast::driver::run_mls();
}
