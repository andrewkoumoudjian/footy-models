# `source-transfermarkt` crate

Own ingestion of authorized Transfermarkt-derived inputs and normalization into source records.

- Do not make unauthorized scraping or prohibited ML-use of Transfermarkt content a required code path.
- Preferred adapters are authorized/local CSV, JSON, database dumps, licensed APIs, or exports.
- Preserve exact external player/club IDs, observation dates, transfer dates, transfer kind, reported fee, loan fee, market value, contract data, and provenance.
- Keep `Permanent`, `Free`, `Loan`, options/obligations, youth, and return-from-loan movements distinct.
- Do not calibrate Transfermarkt values to paid fees inside this crate; `market` owns that logic.
- A missing/undisclosed fee is `None`, not zero.
- Never backfill a market-value observation from a later date.
- Add fixtures for missing fees, loans, duplicate transfers, currency parsing, and historical value sequences.
