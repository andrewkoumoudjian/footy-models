# `ingest` app

Orchestrate source imports, validation, identity staging, and canonical event-store writes.

- Never train or score models here.
- Support local/authorized source inputs first; remote retrieval is optional adapter functionality.
- Validate schema, provenance, duplicate keys, dates, currencies, and coverage before committing canonical records.
- Make ingestion idempotent where possible.
- Partial failures must not leave a dataset version appearing complete.
- Emit source/coverage summaries and unresolved identity-link candidates for review.
