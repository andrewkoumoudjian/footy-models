#![forbid(unsafe_code)]
//! Shared match-forecast driver library used by this package's thin binaries:
//! `footy-matchforecast` (StatsBomb fixture driver), `footy-matchforecast-mls`
//! (MLS forecast) and `footy-matchforecast-pl` (Premier League registered-squad
//! + FM26 Monte Carlo forecast).

pub mod driver;
pub mod historical;
