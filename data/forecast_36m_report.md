# Footy-Models — 36-Month Value Forecast: U23, TM value €400k–€1.2M, Top-100 by % Gain

## 0. Executive summary

- **Scored pool**: all players aged **< 23** with Transfermarkt value **€400,000–€1,200,000**, FM attributes present (`attr_source` in {db, derived}; the 23 `staff` rows are excluded). **948 players scored** out of the 971-row pool (849 db attrs / 99 derived / 23 staff excluded).
- **Model**: deployed v2 forecast artifact `data/forecast/model.bin`, SHA-256 `1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f` (byte-identical after the run), ridge = 1,000,000, seed 42, 47-feature causal schema, trained through 2025-06-12, walk-forward Spearman IC 0.5063 (pooled OOF) / real forward-test IC 0.5100 (as-of 2025-11-06 → realized TM through 2026-06-12).
- **36-month view**: model-consistent compounding of the 12-month log forecast: `predicted_36m_mv = current_mv * exp(3 * mu_12)` with `mu_12 = ln(predicted_12m_mv / current_mv)`; ranking by `gap_pct_36m = exp(3 * mu_12) - 1` (descending).
- **Uncertainty**: 1,000-path Monte Carlo per player (Rust `apps/mc` + `crates/forecast`), 36 monthly steps, seed 42; annual vol = v2 conformal radius 0.6009 / 1.2816 ≈ **0.4688**; drift = the model's annualized 12m log forecast. Top-10 fan charts + top-25 basket on a **white background** (`artifacts/mc_36m_white/`).
- **Costs**: tables below are **gross of transaction costs**; a 15 bps cost applies wherever a portfolio is evaluated (see Limitations).

## 1. Methodology

### 1.1 Model (used exactly as deployed — no retraining, no re-sweep)

| Item | Value |
|---|---|

| Artifact | `data/forecast/model.bin` |

| SHA-256 | `1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f` |

| Ridge λ | 1,000,000 |

| Seed | 42 |

| Schema | 47-feature causal (standardized + paired missingness indicators → 94 model inputs) |

| Trained through | 2025-06-12 |

| Walk-forward Spearman IC | 0.5063 (pooled OOF) |

| Real forward-test IC | 0.5100 (as-of 2025-11-06 → realized TM through 2026-06-12) |

| Conformal 80% radius (12m) | 0.6009 → annual σ = 0.6009 / 1.2816 ≈ 0.4688 |


### 1.2 Candidate pool

- Source: `data/fm_tm_joined.csv` (14,783 rows; **merging is done — not re-merged**).
- Filter: `age < 23` AND `400000 <= current_mv <= 1200000` AND `attr_source != 'staff'`.
- Pool count: **971** players (849 db attrs / 99 derived / 23 staff). The 23 staff rows are excluded → **948 scored**; db vs derived stays flagged in every output.
- No PA filter on this run (U23 + value band only, per spec).
- Missing data is not zero: absent features are NA + missingness indicators; censored/absent players are dropped, not zero-filled.

### 1.3 Scoring (existing primitives)

- Candidates: `data/forecast_36m/candidates.tsv` + `candidates_meta.tsv` built with the exact `build_forecast_dataset.py` feature extractor (as-of each player's latest Transfermarkt valuation date, strictly-prior observations only).
- Materialize: `footy-train forecast-materialize-candidates --input data/forecast_36m/candidates.tsv --dataset-version 1 --feature-set-version 1 --output data/forecast_36m/candidates_matrix.bin` (948 rows, 47 features).
- Score: `footy-score forecast --model data/forecast/model.bin --candidates data/forecast_36m/candidates_matrix.bin --meta data/forecast_36m/candidates_meta.tsv --top-n 1000 --min-pa 0 --max-mv 1200000 --rows 0 --output data/forecast_36m_all.csv` → all 948 scored players (12m columns retained: `predicted_12m_mv`, `gap_eur`, `gap_pct`, conformal `interval_10`/`interval_90`).

### 1.4 36-month compounding (model-consistent)

```
mu_12             = ln(predicted_12m_mv / current_mv)     # model 12-month log ratio
predicted_36m_mv  = current_mv * exp(3 * mu_12)
gap_pct_36m       = exp(3 * mu_12) - 1                       # rank key, descending
gap_eur_36m       = predicted_36m_mv - current_mv
```

The 12m conformal interval columns are kept as-is (they are 12m; 36m chart uncertainty comes from the MC fan).

### 1.5 36-month Monte Carlo (Rust `apps/mc`, `crates/forecast`)

- Drift: `mu_ann = mu_12` (annualized model forecast); vol: `sigma_ann = 0.4688` (v2 conformal radius).
- 1,000 paths per player, 36 monthly steps, seed 42. Driver noise channels (development/performance) off → dispersion = forecast-model uncertainty; transfer hazard 25%/yr + contract bookkeeping from `FootballPathModel`.
- Determinism verified: re-running `footy-mc run` reproduces `paths_long.tsv` / `paths_basket.tsv` / `stats.tsv` byte-identically.
- Charts on **white background** (`#ffffff` figure/axes, dark-gray text, light grid), saved under `artifacts/mc_36m_white/`.

## 2. Pool and scored overview

| Metric | Value |
|---|---|

| Pool rows (pre-screen) | 971 (849 db / 99 derived / 23 staff) |

| Scored rows (`data/forecast_36m_all.csv`) | 948 |

| — db attrs | 849 |

| — derived attrs | 99 |

| Age range | 17.0 – 23.0 |

| MV range (€) | 400,000 – 1,200,000 |

| Positions | MF=398, FW=294, DF=163, GK=93 |

| 12m forecast mean (€) | 899,559 |

| 36m % gain mean | 138.7% |

| 36m % gain median | 123.6% |

| 36m € gap mean (€) | 929,004 |



## 3. Top-10 profiles with Monte Carlo statistics

MC stats from the Rust simulation (1,000 paths, seed 42). `P(≥1.5×)`/`P(≥2×)` are the share of paths ending at ≥1.5×/≥2.0× the current value; `P(transfer 3y)` is the share of paths with a transfer event over 36 months.

### 1. Joyeux Masanka Bungi

- **Club / pos / age**: RB Leipzig · MF · 19.4 yrs  |  **CA/PA**: 92/95  |  **attrs**: db
- **Now**: €400,000  →  **12m forecast**: €688,804  |  **36m forecast (deterministic compounding)**: €2.04M  |  **36m % gain**: 410.6%
- **MC 36m (1,000 paths, seed 42)**: mean €2.74M, median €1.91M, 10–90% [€682,471, €5.53M]; P(≥1.5×) 93%, P(≥2×) 86%, P(transfer 3y) 59%
- **Chart**: `artifacts/mc_36m_white/chart_player_01.png`

### 2. Marvyn Muzungu

- **Club / pos / age**: Ajax Amsterdam · MF · 18.5 yrs  |  **CA/PA**: 100/130  |  **attrs**: db
- **Now**: €400,000  →  **12m forecast**: €677,511  |  **36m forecast (deterministic compounding)**: €1.94M  |  **36m % gain**: 385.9%
- **MC 36m (1,000 paths, seed 42)**: mean €2.61M, median €1.82M, 10–90% [€649,913, €5.27M]; P(≥1.5×) 92%, P(≥2×) 85%, P(transfer 3y) 59%
- **Chart**: `artifacts/mc_36m_white/chart_player_02.png`

### 3. Andrei Chirilă

- **Club / pos / age**: Football Club Cincinnati · DF · 17.8 yrs  |  **CA/PA**: 75/105  |  **attrs**: derived
- **Now**: €500,000  →  **12m forecast**: €843,429  |  **36m forecast (deterministic compounding)**: €2.40M  |  **36m % gain**: 380.0%
- **MC 36m (1,000 paths, seed 42)**: mean €3.22M, median €2.25M, 10–90% [€802,617, €6.51M]; P(≥1.5×) 92%, P(≥2×) 85%, P(transfer 3y) 59%
- **Chart**: `artifacts/mc_36m_white/chart_player_03.png`

### 4. Andrea Natali

- **Club / pos / age**: AZ Alkmaar · DF · 18.4 yrs  |  **CA/PA**: 95/125  |  **attrs**: db
- **Now**: €400,000  →  **12m forecast**: €673,517  |  **36m forecast (deterministic compounding)**: €1.91M  |  **36m % gain**: 377.4%
- **MC 36m (1,000 paths, seed 42)**: mean €2.56M, median €1.79M, 10–90% [€638,649, €5.18M]; P(≥1.5×) 92%, P(≥2×) 84%, P(transfer 3y) 59%
- **Chart**: `artifacts/mc_36m_white/chart_player_04.png`

### 5. Mateo Pérez Curci

- **Club / pos / age**: Club Atlético Independiente de Avellaneda · FW · 20.4 yrs  |  **CA/PA**: 109/120  |  **attrs**: derived
- **Now**: €1.00M  →  **12m forecast**: €1.68M  |  **36m forecast (deterministic compounding)**: €4.77M  |  **36m % gain**: 376.9%
- **MC 36m (1,000 paths, seed 42)**: mean €6.40M, median €4.47M, 10–90% [€1.60M, €12.93M]; P(≥1.5×) 92%, P(≥2×) 84%, P(transfer 3y) 59%
- **Chart**: `artifacts/mc_36m_white/chart_player_05.png`

### 6. Alfie Dorrington

- **Club / pos / age**: Aberdeen FC · MF · 21.1 yrs  |  **CA/PA**: 102/124  |  **attrs**: db
- **Now**: €500,000  →  **12m forecast**: €840,106  |  **36m forecast (deterministic compounding)**: €2.37M  |  **36m % gain**: 374.3%
- **MC 36m (1,000 paths, seed 42)**: mean €3.18M, median €2.22M, 10–90% [€793,302, €6.43M]; P(≥1.5×) 91%, P(≥2×) 84%, P(transfer 3y) 59%
- **Chart**: `artifacts/mc_36m_white/chart_player_06.png`

### 7. Lisandro Gerónimo Piñero

- **Club / pos / age**: Club Atlético Banfield · FW · 18.9 yrs  |  **CA/PA**: 117/133  |  **attrs**: db
- **Now**: €400,000  →  **12m forecast**: €670,654  |  **36m forecast (deterministic compounding)**: €1.89M  |  **36m % gain**: 371.3%
- **MC 36m (1,000 paths, seed 42)**: mean €2.53M, median €1.77M, 10–90% [€630,655, €5.11M]; P(≥1.5×) 91%, P(≥2×) 84%, P(transfer 3y) 59%
- **Chart**: `artifacts/mc_36m_white/chart_player_07.png`

### 8. Mamadou Lamine Sadio

- **Club / pos / age**: FC Nordsjaelland · FW · 18.5 yrs  |  **CA/PA**: 102/115  |  **attrs**: db
- **Now**: €850,000  →  **12m forecast**: €1.42M  |  **36m forecast (deterministic compounding)**: €3.94M  |  **36m % gain**: 363.9%
- **MC 36m (1,000 paths, seed 42)**: mean €5.29M, median €3.70M, 10–90% [€1.32M, €10.70M]; P(≥1.5×) 90%, P(≥2×) 83%, P(transfer 3y) 59%
- **Chart**: `artifacts/mc_36m_white/chart_player_08.png`

### 9. Facundo Valentín Jainikoski

- **Club / pos / age**: Asociación Atlética Argentinos Juniors · FW · 18.2 yrs  |  **CA/PA**: 105/137  |  **attrs**: db
- **Now**: €1.20M  →  **12m forecast**: €2.00M  |  **36m forecast (deterministic compounding)**: €5.54M  |  **36m % gain**: 361.6%
- **MC 36m (1,000 paths, seed 42)**: mean €7.44M, median €5.19M, 10–90% [€1.85M, €15.03M]; P(≥1.5×) 90%, P(≥2×) 83%, P(transfer 3y) 59%
- **Chart**: `artifacts/mc_36m_white/chart_player_09.png`

### 10. Nevio Scherrer

- **Club / pos / age**: FC St. Gallen 1879 · FW · 18.2 yrs  |  **CA/PA**: 82/93  |  **attrs**: db
- **Now**: €600,000  →  **12m forecast**: €992,636  |  **36m forecast (deterministic compounding)**: €2.72M  |  **36m % gain**: 352.8%
- **MC 36m (1,000 paths, seed 42)**: mean €3.65M, median €2.55M, 10–90% [€909,355, €7.37M]; P(≥1.5×) 90%, P(≥2×) 82%, P(transfer 3y) 59%
- **Chart**: `artifacts/mc_36m_white/chart_player_10.png`

## 4. Top-100 by expected 36-month % gain (gross of costs)

Ranked by `gap_pct_36m` descending (tie-break `player_id` ascending). All rows: age < 23, €400k ≤ current_mv ≤ €1.2M, attr_source ≠ staff. Amounts in EUR; `gap_eur`/`gap_pct` are the **36-month** gap. Gross of the 15 bps transaction cost (applies at portfolio evaluation).

| # | player_id | Name | Club | Comp | Pos | Age | Current MV | 12m fcast | gap_eur (36m) | gap_pct (36m) | attrs |

|---|---|---|---|---|---|---|---|---|---|---|---|---|

| 1 | 990146 | Joyeux Masanka Bungi | RB Leipzig | bundesliga | MF | 19.4 | €400k | €689k | €1.64M | 410.6% | db |

| 2 | 1233650 | Marvyn Muzungu | Ajax Amsterdam | eredivisie | MF | 18.5 | €400k | €678k | €1.54M | 385.9% | db |

| 3 | 1073380 | Andrei Chirilă | Football Club Cincinna | major-league-socce | DF | 17.8 | €500k | €843k | €1.90M | 380.0% | derived |

| 4 | 1058353 | Andrea Natali | AZ Alkmaar | eredivisie | DF | 18.4 | €400k | €674k | €1.51M | 377.4% | db |

| 5 | 1298516 | Mateo Pérez Curci | Club Atlético Independ | torneo-apertura | FW | 20.4 | €1.00M | €1.68M | €3.77M | 376.9% | derived |

| 6 | 796300 | Alfie Dorrington | Aberdeen FC | scottish-premiersh | MF | 21.1 | €500k | €840k | €1.87M | 374.3% | db |

| 7 | 1301427 | Lisandro Gerónimo Piñero | Club Atlético Banfield | torneo-apertura | FW | 18.9 | €400k | €671k | €1.49M | 371.3% | db |

| 8 | 1133737 | Mamadou Lamine Sadio | FC Nordsjaelland | superliga | FW | 18.5 | €850k | €1.42M | €3.09M | 363.9% | db |

| 9 | 1310744 | Facundo Valentín Jainikoski | Asociación Atlética Ar | torneo-apertura | FW | 18.2 | €1.20M | €2.00M | €4.34M | 361.6% | db |

| 10 | 1263348 | Nevio Scherrer | FC St. Gallen 1879 | super-league | FW | 18.2 | €600k | €993k | €2.12M | 352.8% | db |

| 11 | 1410674 | Mamady Cissé | Clube Atlético Mineiro | campeonato-brasile | MF | 19.4 | €1.00M | €1.65M | €3.53M | 352.7% | db |

| 12 | 1421514 | Yannick Emilio Geiger | Allmänna Idrottsklubbe | allsvenskan | MF | 19.0 | €600k | €991k | €2.10M | 350.6% | db |

| 13 | 911224 | Alessandro Gianni Romano | Associazione Sportiva  | serie-a | MF | 20.0 | €500k | €823k | €1.73M | 345.4% | db |

| 14 | 697280 | George Earthy | West Ham United | premier-league | MF | 21.8 | €1.00M | €1.64M | €3.44M | 344.0% | db |

| 15 | 1108464 | Reggie Spencer Walsh | Chelsea FC | premier-league | FW | 17.6 | €500k | €821k | €1.71M | 342.9% | db |

| 16 | 1324003 | Joaquín Freitas | Club Atlético River Pl | torneo-apertura | FW | 19.5 | €1.00M | €1.64M | €3.42M | 342.4% | db |

| 17 | 1421433 | Zadok Yohanna | Allmänna Idrottsklubbe | allsvenskan | FW | 19.0 | €500k | €815k | €1.67M | 333.2% | db |

| 18 | 1139402 | Jerónimo Russo | Club Atlético Newell’s | torneo-apertura | FW | 21.0 | €1.00M | €1.63M | €3.32M | 332.1% | derived |

| 19 | 1085591 | Bendito Boanova Mantato | Manchester United | premier-league | FW | 18.4 | €400k | €651k | €1.33M | 331.4% | db |

| 20 | 937720 | Tiago Dias Freitas | SL Benfica | liga-portugal | MF | 19.8 | €550k | €895k | €1.82M | 330.3% | db |

| 21 | 1342148 | Romeo Leandersson | Mjällby Almänna Idrott | allsvenskan | MF | 17.9 | €600k | €973k | €1.96M | 326.3% | db |

| 22 | 1049906 | Ben Brannan | Kilmarnock FC | scottish-premiersh | MF | 19.4 | €400k | €648k | €1.30M | 325.0% | db |

| 23 | 979850 | Andrea Cogoni | Cagliari Calcio | serie-a | MF | 19.9 | €400k | €648k | €1.30M | 324.9% | db |

| 24 | 984593 | Triston Rowe | Aston Villa | premier-league | MF | 19.7 | €500k | €809k | €1.62M | 323.8% | db |

| 25 | 1054467 | Cooper Sanchez | Atlanta United Footbal | major-league-socce | FW | 18.2 | €1.00M | €1.61M | €3.21M | 321.0% | derived |

| 26 | 985544 | Benjamin Tomaso Broggio | Falkirk FC | scottish-premiersh | FW | 19.4 | €400k | €646k | €1.28M | 320.8% | db |

| 27 | 1138091 | Samuel Kováčik | Legia Warszawa | pko-bp-ekstraklasa | FW | 19.0 | €1.00M | €1.61M | €3.20M | 320.4% | db |

| 28 | 1083702 | Niko Šepić | NK Istra 1961 | supersport-hnl | MF | 20.3 | €400k | €645k | €1.27M | 318.4% | db |

| 29 | 1428280 | Natan Ława | Pogon Szczecin | pko-bp-ekstraklasa | FW | 17.0 | €500k | €803k | €1.57M | 314.3% | db |

| 30 | 1008429 | Justin Ellis | Orlando City Soccer Cl | major-league-socce | FW | 19.1 | €500k | €802k | €1.56M | 312.8% | db |

| 31 | 1121428 | Marko Zebić | GNK Dinamo Zagreb | supersport-hnl | DF | 19.4 | €500k | €801k | €1.56M | 311.4% | db |

| 32 | 924859 | Rory Wilson | Sportklub Puntigamer S | bundesliga | FW | 20.4 | €400k | €640k | €1.23M | 308.7% | db |

| 33 | 1213079 | Giovanni Ismael Cantizano | Club Atlético Rosario  | torneo-apertura | MF | 19.2 | €1.00M | €1.60M | €3.08M | 308.1% | derived |

| 34 | 1006916 | Jonathan Vonheim Norbye | Fredrikstad Fotballklu | eliteserien | DF | 19.2 | €700k | €1.11M | €2.12M | 302.9% | db |

| 35 | 801446 | Diogo José de Nazaré e Jesus F | SL Benfica | liga-portugal | GK | 19.3 | €800k | €1.27M | €2.42M | 302.4% | db |

| 36 | 930768 | Matteo Spinaccè | Inter Milan | serie-a | FW | 19.9 | €1.00M | €1.58M | €2.96M | 296.3% | db |

| 37 | 1335777 | Simão Ambrósio Pedro | FK Zeleznicar Pancevo | super-liga-srbije | FW | 19.8 | €700k | €1.11M | €2.07M | 295.7% | db |

| 38 | 1282697 | Lautaro Ángel Ezequiel Montene | Club Atlético San Lore | torneo-apertura | FW | 21.3 | €1.00M | €1.58M | €2.92M | 291.6% | derived |

| 39 | 976207 | Federico Serra | Società Sportiva Lazio | serie-a | FW | 20.4 | €450k | €709k | €1.31M | 291.3% | db |

| 40 | 1011140 | Jack Davidson Fletcher | Manchester United | premier-league | FW | 19.2 | €1.00M | €1.58M | €2.91M | 290.9% | db |

| 41 | 972475 | Matteo Lavelli | Inter Milan | serie-a | MF | 19.5 | €450k | €709k | €1.31M | 290.6% | derived |

| 42 | 1285240 | Felipe Bussio | Club Atlético Platense | torneo-apertura | MF | 22.0 | €400k | €629k | €1.16M | 289.1% | db |

| 43 | 1044831 | David Miguel Madeira Moreira | Sporting CP | liga-portugal | DF | 22.1 | €800k | €1.26M | €2.31M | 288.4% | db |

| 44 | 1070983 | Can Armando Güner | Galatasaray | super-lig | FW | 18.4 | €500k | €785k | €1.44M | 287.5% | db |

| 45 | 1094582 | Kparobo Arierhi | Lillestrøm Sportsklubb | eliteserien | FW | 19.4 | €500k | €785k | €1.43M | 286.6% | db |

| 46 | 1296393 | Agustín Alejo Ladstatter | Club Atlético San Lore | torneo-apertura | GK | 21.0 | €400k | €624k | €1.12M | 280.3% | derived |

| 47 | 1133978 | Sékou Koné | Football Club Lausanne | super-league | MF | 20.4 | €400k | €624k | €1.12M | 279.3% | db |

| 48 | 1255754 | Noah Klas Tolf | Idrottsföreningen Kamr | allsvenskan | MF | 20.8 | €1.00M | €1.56M | €2.79M | 278.9% | db |

| 49 | 1133013 | Matija Subotić | NK Lokomotiva Zagreb | supersport-hnl | MF | 18.5 | €850k | €1.32M | €2.36M | 277.2% | db |

| 50 | 1457367 | Ali Badra Diabaté | Samsunspor | super-lig | MF | 19.5 | €1.00M | €1.55M | €2.76M | 275.6% | db |

| 51 | 1125079 | Shafiq Bello Nandja | Hamburger SV | bundesliga | DF | 19.4 | €500k | €777k | €1.38M | 275.1% | db |

| 52 | 1235196 | Eder López | FC Juárez | liga-mx-clausura | FW | 20.4 | €1.00M | €1.55M | €2.75M | 275.1% | derived |

| 53 | 1072028 | Oscar Olivier | Standard Liège | jupiler-pro-league | FW | 19.0 | €500k | €777k | €1.37M | 274.6% | db |

| 54 | 1111922 | Rodolfo Aloko | Charlotte Football Clu | major-league-socce | MF | 19.5 | €1.00M | €1.55M | €2.74M | 274.3% | db |

| 55 | 1245665 | Ian James | Sporting Kansas City | major-league-socce | DF | 18.0 | €800k | €1.24M | €2.19M | 273.8% | db |

| 56 | 1134255 | Fouad Zahouani | Free Agent | Unknown | DF | 20.2 | €1.00M | €1.55M | €2.72M | 272.4% | db |

| 57 | 1290658 | Leo Håkan Östman | Idrottsföreningen Elfs | allsvenskan | MF | 20.2 | €600k | €930k | €1.63M | 272.3% | db |

| 58 | 999129 | Nolan Michael Miller | Orlando City Soccer Cl | major-league-socce | DF | 21.9 | €500k | €774k | €1.36M | 271.6% | db |

| 59 | 1145213 | Giovanni de Abreu | Perth Glory | a-league-men | MF | 19.2 | €400k | €619k | €1.08M | 270.3% | derived |

| 60 | 1037572 | Daniil Chevardin | "Футбольный клуб ""Лок | premier-liga | MF | 19.8 | €500k | €773k | €1.35M | 269.9% | db |

| 61 | 1448027 | Gabrijel Šivalec | Slaven Belupo Koprivni | supersport-hnl | FW | 18.1 | €500k | €772k | €1.34M | 267.9% | db |

| 62 | 1341717 | Marcus Haagensen Reed | Aalesunds Fotballklubb | eliteserien | MF | 18.3 | €500k | €771k | €1.34M | 267.0% | derived |

| 63 | 1010300 | David Vazquez | San Diego Football Clu | major-league-socce | MF | 20.3 | €1.00M | €1.54M | €2.67M | 266.8% | db |

| 64 | 1172422 | Filip Olov Öhman | BK Häcken | allsvenskan | MF | 18.4 | €1.00M | €1.54M | €2.65M | 264.6% | db |

| 65 | 902701 | João Tomás de Sousa Moreira Cr | Free Agent | Unknown | MF | 21.0 | €450k | €692k | €1.19M | 264.4% | db |

| 66 | 983554 | João Carvalho Fonseca | SL Benfica | liga-portugal | DF | 19.8 | €600k | €922k | €1.58M | 263.3% | db |

| 67 | 1353562 | Thomas Lopes | Servette FC | super-league | DF | 19.4 | €800k | €1.23M | €2.10M | 262.6% | derived |

| 68 | 1009603 | Sven Šunta | GNK Dinamo Zagreb | supersport-hnl | GK | 19.1 | €400k | €614k | €1.04M | 261.1% | db |

| 69 | 866250 | Seydou Fini | Genoa CFC | serie-a | FW | 20.0 | €1.00M | €1.53M | €2.59M | 259.4% | db |

| 70 | 1163767 | Pascal Mozie | Legia Warszawa | pko-bp-ekstraklasa | MF | 18.1 | €600k | €918k | €1.55M | 258.3% | db |

| 71 | 923531 | Oliver Jordan Hagen | Hammarby IF | allsvenskan | FW | 20.0 | €500k | €765k | €1.29M | 258.3% | db |

| 72 | 1183410 | Charlie Steven Brian Pavey | Allmänna Idrottsklubbe | allsvenskan | MF | 18.0 | €400k | €612k | €1.03M | 258.0% | db |

| 73 | 1269429 | Diego Rodrigo Porcel | Asociación Atlética Ar | torneo-apertura | FW | 21.4 | €800k | €1.22M | €2.06M | 257.3% | db |

| 74 | 1043574 | György Viktor Kun | CFR Cluj | superliga | MF | 21.9 | €400k | €610k | €1.02M | 254.0% | db |

| 75 | 1075884 | João Soares Aragão | SC Braga | liga-portugal | FW | 18.2 | €500k | €761k | €1.26M | 252.5% | db |

| 76 | 944140 | Trent Toure Kone-Doherty | Liverpool FC | premier-league | FW | 20.0 | €600k | €913k | €1.51M | 252.3% | db |

| 77 | 1085031 | Elia Plicco | Parma Calcio 1913 | serie-a | FW | 19.4 | €500k | €760k | €1.26M | 251.7% | db |

| 78 | 1141848 | Savo Radanović | Red Star Belgrade | super-liga-srbije | GK | 17.3 | €700k | €1.06M | €1.75M | 250.5% | db |

| 79 | 925417 | Tiago Parreira Parente | GD Estoril Praia | liga-portugal | MF | 22.4 | €500k | €759k | €1.25M | 250.4% | db |

| 80 | 979773 | Téo Barišić | HNK Rijeka | supersport-hnl | DF | 21.7 | €700k | €1.06M | €1.75M | 250.2% | db |

| 81 | 944095 | Olle Samuelsson | BK Häcken | allsvenskan | MF | 22.1 | €400k | €607k | €1,000k | 250.0% | db |

| 82 | 856593 | Tomás Uriel Porra | Club Atlético Barracas | torneo-apertura | MF | 22.4 | €1.20M | €1.82M | €2.99M | 249.1% | db |

| 83 | 1247243 | Gleb Popolitov | PFK CSKA Moskva | premier-liga | FW | 18.8 | €500k | €757k | €1.23M | 247.0% | db |

| 84 | 1026255 | Saná Eusébio Mango Fernandes | Società Sportiva Lazio | serie-a | FW | 20.3 | €800k | €1.21M | €1.97M | 246.0% | db |

| 85 | 1204043 | Ola Visted | Viking Fotballklubb | eliteserien | MF | 21.2 | €600k | €907k | €1.47M | 245.1% | db |

| 86 | 848753 | Tommy Hogan Setford | Arsenal FC | premier-league | GK | 20.2 | €500k | €755k | €1.22M | 244.5% | db |

| 87 | 1102159 | José Augusto Silva | FC Arouca | liga-portugal | MF | 20.8 | €500k | €754k | €1.22M | 243.4% | db |

| 88 | 1061068 | Joel Ruiz | FC St. Gallen 1879 | super-league | MF | 19.6 | €400k | €603k | €971k | 242.8% | derived |

| 89 | 1365590 | Reid Roberts | San Jose Earthquakes | major-league-socce | DF | 22.5 | €1.00M | €1.51M | €2.42M | 242.1% | db |

| 90 | 1283982 | Jeremías Daniel Merlo | Club de Gimnasia y Esg | torneo-apertura | MF | 20.0 | €700k | €1.05M | €1.67M | 238.4% | db |

| 91 | 1138260 | Olivier Mambwa | BSC Young Boys | super-league | DF | 17.5 | €1.00M | €1.50M | €2.38M | 237.6% | db |

| 92 | 932920 | Stefan Gudelj | Red Star Belgrade | super-liga-srbije | DF | 20.1 | €1.00M | €1.50M | €2.35M | 234.9% | db |

| 93 | 1380977 | Samba Konaté | RB Leipzig | bundesliga | GK | 17.3 | €1.00M | €1.49M | €2.33M | 233.1% | db |

| 94 | 1153297 | Adrien Llukes | FC Sion | super-league | FW | 17.9 | €500k | €746k | €1.16M | 231.9% | db |

| 95 | 1149569 | Julián Darío Ceballos | Club Atlético Gimnasia | torneo-apertura | FW | 21.7 | €450k | €671k | €1.04M | 231.1% | db |

| 96 | 990008 | Wassim Bouziane | AZ Alkmaar | eredivisie | FW | 19.5 | €450k | €671k | €1.04M | 231.1% | db |

| 97 | 1118293 | Fynn Schenten | 1.FC Köln | bundesliga | FW | 18.7 | €1.00M | €1.49M | €2.31M | 230.7% | db |

| 98 | 1133977 | Issa Traoré | Bayer 04 Leverkusen | bundesliga | FW | 18.6 | €800k | €1.19M | €1.85M | 230.6% | db |

| 99 | 1404854 | Sebastian Despotovski | Perth Glory | a-league-men | FW | 20.5 | €400k | €595k | €920k | 229.9% | derived |

| 100 | 985546 | Callum Latif Olusesi | Tottenham Hotspur | premier-league | MF | 19.3 | €500k | €742k | €1.14M | 227.2% | db |



## 5. Basket overview (top-25 equal weight)

Equal-weight basket of the top 25 by expected 36m % gain, each player normalized to 1.0 (index start = 100), 1,000 paths, seed 42.

| Basket stat (36m) | Value |
|---|---|

| Mean index | 606 |

| Median index | 423 |

| 10–90% range | [151, 1225] |

| P(≥1.5×) | 90% |

| P(≥2×) | 82% |

| P(≤0.5×) | 0.6% |

- Chart: `artifacts/mc_36m_white/chart_overview.png`


## 6. White-background chart inventory

All charts use the white palette (`#ffffff` figure/axes, text `#1f2328`/`#57606a`, grid `#d0d7de`, paths `#9cc3f5`, mean `#c62828`, median dashed dark gray, 10–90% band `#d6e6fb`), subtitle below the x-axis.

| File | Content |
|---|---|

| `artifacts/mc_36m_white/chart_overview.png` | Top-25 basket fan (index = 100) |

| `artifacts/mc_36m_white/chart_player_01.png` | #1 player fan (1,000 paths, 36 months) |

| `artifacts/mc_36m_white/chart_player_02.png` | #2 player fan (1,000 paths, 36 months) |

| `artifacts/mc_36m_white/chart_player_03.png` | #3 player fan (1,000 paths, 36 months) |

| `artifacts/mc_36m_white/chart_player_04.png` | #4 player fan (1,000 paths, 36 months) |

| `artifacts/mc_36m_white/chart_player_05.png` | #5 player fan (1,000 paths, 36 months) |

| `artifacts/mc_36m_white/chart_player_06.png` | #6 player fan (1,000 paths, 36 months) |

| `artifacts/mc_36m_white/chart_player_07.png` | #7 player fan (1,000 paths, 36 months) |

| `artifacts/mc_36m_white/chart_player_08.png` | #8 player fan (1,000 paths, 36 months) |

| `artifacts/mc_36m_white/chart_player_09.png` | #9 player fan (1,000 paths, 36 months) |

| `artifacts/mc_36m_white/chart_player_10.png` | #10 player fan (1,000 paths, 36 months) |


## 7. Reproducibility and limitations

- Seed 42 used for model training, scoring determinism, and all MC paths; `footy-mc` re-run reproduces identical artifacts (verified byte-for-byte).
- Deterministic ranking (gap_pct_36m desc, player_id asc); identical inputs → identical outputs.
- `data/forecast/model.bin` untouched: SHA-256 matches `1d023e96…` before and after the run.
- `data/forecast_u23_top100.csv`, `data/forecast_u23_top100_v2.csv`, `data/forecast/model.bin` not overwritten.
- **attr_source honesty**: db vs derived flagged in every output; staff rows excluded; no zero-fill of missing data (censored/absent players dropped).
- Tables are gross of transaction costs; a 15 bps cost applies wherever a portfolio is evaluated (top-N tables may be gross, labeled as such — they are here).
- Monte Carlo dispersion equals the forecast model's conformal uncertainty (driver noise off); transfer hazard is a fixed 25%/yr parameter, not a re-estimated per-player hazard.
- Transfermarkt DuckDB accessed read-only; `fm_tm_joined.csv` not re-merged.
