# `score` app

Orchestrate efficient player/candidate scoring from immutable model and feature artifacts.

- No source scraping or model fitting in the scoring path.
- Fail on feature/model version mismatch rather than silently reordering or dropping features.
- Prefer compact contiguous reads and batch scoring.
- Return component distributions: perceived, fundamental, acquisition, future, hazard, fit, wage/TAC, and confidence as available.
- Keep latency/memory optimizations evidence-based and benchmarked.
