# CF Montréal Scouting Report: Top-20 Psych-Fit, Positive-Yield Value-Spike Targets

**As-of vintage:** TM valuations through 2026-06-12; FM26 attributes (Nov 2025); SofaScore season statistics current at fetch time (Aug 2026). **Seed:** 42 everywhere (shortlist z-scores, Monte Carlo). **Club:** CF Montréal (MLS, club_id 4078).

---

## 1. Executive summary

CF Montréal is a bridge club: it signs young players, develops them and sells them on to stronger leagues (the alumni evidence in section 3). This report ranks the top 20 candidates in the CFM scout basket (age < 23, EUR 400k to 2.5M market value) on a combination of **psychological fit** (calibrated on successful alumni), **positive 12-month yield** (v2 forecast), **spike potential** (v3 expectile-0.9 tail score) and **squad complementarity** (the current squad has no left winger and only two left-backs). Every shortlisted player has strictly positive forecast yield and a tail score in the top 40% of the basket.

**Headline table (ranked by combined score; MC numbers are the spike run, seed 42, 1000 paths, annual vol 0.4688):**

| # | Player | Club | Pos | Age | Value | Psych fit | Tail | 12m yield | Gap | SofaScore rating | MC median 36m | P(>=1.5x) |
|---|--------|------|-----|-----|-------|-----------|------|-----------|-----|------------------|---------------|-----------|
| 1 | Lisandro Piñero | Club Atlético Banfield | FW | 18.9 | EUR 400k | 0.627 | 1.12 | 68% | LW | 6.7 | EUR 10.48M | 100% |
| 2 | Noah Tolf | Idrottsföreningen Kamraterna Göteborg | MF | 20.8 | EUR 1.00M | 0.664 | 1.04 | 56% | LB | 6.75 | EUR 20.66M | 100% |
| 3 | Yannick Geiger | Allmänna Idrottsklubben | MF | 19.0 | EUR 600k | 0.643 | 1.08 | 65% | CM | 6.83 | EUR 14.09M | 100% |
| 4 | Jeremías Merlo | Club de Gimnasia y Esgrima La Plata | MF | 20.0 | EUR 700k | 0.672 | 1.02 | 50% | LW | 6.47 | EUR 13.87M | 100% |
| 5 | Niko Sepic | NK Istra 1961 | MF | 20.3 | EUR 400k | 0.598 | 1.06 | 61% | LB | 6.68 | EUR 8.91M | 100% |
| 6 | George Earthy | West Ham United | MF | 21.8 | EUR 1.00M | 0.641 | 1.08 | 64% | AM | 6.6 | EUR 23.17M | 100% |
| 7 | Alfie Dorrington | Aberdeen FC | MF | 21.1 | EUR 500k | 0.646 | 1.12 | 68% | CB | 6.47 | EUR 13.15M | 100% |
| 8 | Romeo Arrhenius Leandersson | Mjällby Almänna Idrottsförening | MF | 17.9 | EUR 600k | 0.624 | 1.08 | 62% | CM | 6.61 | EUR 14.04M | 100% |
| 9 | Alessandro Romano | Associazione Sportiva Roma | MF | 20.0 | EUR 500k | 0.597 | 1.10 | 64% | DM | 6.88 | EUR 12.45M | 100% |
| 10 | Andrea Natali | AZ Alkmaar | DF | 18.4 | EUR 400k | 0.619 | 1.13 | 68% | CB | 6.76 | EUR 10.71M | 100% |
| 11 | Tiago Freitas | SL Benfica | MF | 19.8 | EUR 550k | 0.602 | 1.08 | 63% | CM | 6.62 | EUR 13.03M | 100% |
| 12 | Felipe Bussio | Club Atlético Platense | MF | 22.0 | EUR 400k | 0.644 | 1.05 | 57% | DM | 6.6 | EUR 8.48M | 100% |
| 13 | Andrea Cogoni | Cagliari Calcio | MF | 19.9 | EUR 400k | 0.616 | 1.08 | 62% | CB | 7.02 | EUR 9.44M | 100% |
| 14 | Wassim Bouziane | AZ Alkmaar | FW | 19.5 | EUR 450k | 0.602 | 0.98 | 49% | LW | 7.08 | EUR 7.73M | 100% |
| 15 | Diogo Ferreira | SL Benfica | GK | 19.3 | EUR 800k | 0.623 | 1.07 | 59% | GK | 6.85 | EUR 17.99M | 100% |
| 16 | Olivier Mambwa | BSC Young Boys | DF | 17.5 | EUR 1.00M | 0.576 | 1.00 | 50% | LB | 6.14 | EUR 18.25M | 100% |
| 17 | Matija Subotic | NK Lokomotiva Zagreb | MF | 18.5 | EUR 850k | 0.599 | 1.02 | 56% | DM | 6.67 | EUR 16.69M | 100% |
| 18 | Matteo Spinaccè | Inter Milan | FW | 19.9 | EUR 1.00M | 0.628 | 1.05 | 58% | CF | 6.74 | EUR 21.18M | 100% |
| 19 | Adrian Kurd Rønning | Kristiansund Ballklubb | FW | 20.2 | EUR 450k | 0.704 | 0.93 | 43% | AM | 6.54 | EUR 6.76M | 100% |
| 20 | Seydou Fini | Genoa CFC | FW | 20.0 | EUR 1.00M | 0.655 | 1.01 | 53% | RW | 7.06 | EUR 19.16M | 100% |

The full detail for each player is in section 6, Monte Carlo fan charts (base and spike runs) in `artifacts/cfm_mc/charts/` and `artifacts/cfm_mc_spike/charts/`, and the machine-readable shortlist in `data/cfm_scout_top20.csv`.

---

## 2. Methodology

### 2.1 Candidate universe (the CFM scout basket)

From the pre-merged panel `data/cfm_scout_panel.csv` (14,769 rows, read-only, never re-merged) we apply the CFM scout basket filter:

- age < 23 years (floating, as of the 2026-06-12 TM vintage);
- EUR 400,000 <= current market value <= EUR 2,500,000;
- `attr_source != 'staff'` (real FM database attributes only);
- club not CF Montréal (excluded via `cfm_squad.tsv` names and club string match).

The basket contains **1,255 players**. Forecast (f36) coverage is **947/14,769** rows (the 36-month forecast universe is smaller than the full FM x TM join); tail (v3 spike) coverage is 7,962/14,769. Coverage is reported honestly, never zero-filled.

### 2.2 Psych archetype: calibrated on alumni, not opinion

**Success label per alumnus:** `success = current_mv / transfer_fee` from `cfm_alumni.tsv`. Alumni with `success >= 1.5` and a meaningful fee (> EUR 300k) form the reference group (9 alumni); the **top half by success (5 alumni)** is the archetype calibration set:

| Alumnus | Transfer fee | Current value | Success multiple |
|---------|-------------|---------------|------------------|
| Joel Waterman | EUR 430k | EUR 2.00M | 4.65x (calibration set) |
| Nathan-Dylan Saliba | EUR 2.00M | EUR 7.50M | 3.75x (calibration set) |
| Aarón Herrera | EUR 464k | EUR 1.50M | 3.23x (calibration set) |
| Ismaël Koné | EUR 8.00M | EUR 25.00M | 3.12x (calibration set) |
| Raheem Edwards | EUR 345k | EUR 800k | 2.32x (calibration set) |
| Alistair Johnston | EUR 3.50M | EUR 8.00M | 2.29x |
| Djordje Mihailovic | EUR 3.00M | EUR 5.50M | 1.83x |
| Caden Clark | EUR 600k | EUR 1.00M | 1.67x |
| George Campbell | EUR 1.30M | EUR 2.00M | 1.54x |

The archetype vector is the mean of non-missing FM attributes over these 5 alumni (FM26 full attrs, read-only lookup by uid):

| Mental attribute | Archetype (alumni mean) |
|------------------|--------------------------|
| Determination | 11.25 |
| Bravery | 11.20 |
| Consistency | 11.80 |
| ImportantMatches | 8.75 |
| Composure | 12.00 |
| WorkRate | 12.40 |
| Teamwork | 12.00 |
| Leadership | 10.40 |
| Versatility | 10.80 |
| NaturalFitness | 12.75 |
| InjuryProneness | 10.33 |

Hidden personality (Pressure, Professionalism, Ambition, Adaptability, Temperament, Controversy) is present on ~32% of panel rows, but the alumni carry no usable hidden values (Koné's Ambition=0 and Adaptability=1 are FM placeholders outside the 1-20 scale). Per the steering's rule, the hidden-attribute archetype therefore uses the documented **bridge-club prior**: Pressure >= 12, Professionalism >= 12, Temperament 12, Controversy <= 8. InjuryProneness and Controversy are **negative** (lower is better).

### 2.3 psych_fit score (exact formula)

For each candidate and attribute the distance to the archetype is computed in z-units using the basket's per-attribute standard deviation (min 1.0):

```
z_dist = sqrt( mean over available attrs of ((arch[a] - v)/scale[a])^2 )
         where for NEGATIVE attrs (InjuryProneness, Controversy) the signed
         distance is reversed: ((v - arch[a])/scale[a])^2

psych_fit = 1 / (1 + exp(K * (z_dist - Z0))),   K = 1.5, Z0 = 1.1
```

The score is computed over the intersection of available candidate attributes and archetype attributes; `psy_attrs_used` counts how many attributes entered the score (max 15 = 11 mental + 4 hidden). The sigmoid maps the distance to [0,1] and is monotone (higher fit, higher score). **Sanity check:** the calibration alumni themselves score 0.61 to 0.67 (min 0.614, mean 0.670), all >= 0.6 as required.

### 2.4 Yield, spike and combined rank

- **Yield (positive expected appreciation):** `f36_gap_pct > 0` AND `f36_predicted_12m_mv > current_mv` (both required, strictly positive). The shortlist uses only positive-yield players; `yield_12m_pct = 100 * f36_gap_pct`.
- **Spike score:** `tail_tail_score` (v3 expectile-0.9 12-month log-ratio), the primary spike rank; all top-20 carry a real tail score and sit in the top 40% of the basket (>= 0.867) and above the basket spike median (0.845).
- **Combined rank** (documented weights, applied over the coverage-gated pool of 617 players who pass yield, tail-coverage and attribute-coverage gates):

```
combined = z(psych_fit) + 0.5*z(tail_score) + 0.3*z(yield_12m_pct) + 0.4*z(squad_fit)
```

z-scores are over the coverage-gated pool. Selection constraints: the top-20 must be above the basket psych-fit median (0.573) and spike median (0.845), have positive yield, and respect position caps (max 3 per squad slot) so the shortlist stays complementary rather than concentrated.

### 2.5 Squad-fit rules

`squad_fit = positional need + age fit + value-band fit` with:

- **Positional need:** LW and LB = 3 (squad has 0 LW, 2 LB); AM/CM/DM = 2; RW/RM/CF/SS/CB/RB/GK = 1 (overstocked: 6 CB, 5 CF, 4 RB, 4 RW, 3 GK).
- **Age fit:** 17-21 preferred (MLS U22 initiative window): +2 if age < 22, else +1.
- **Value band:** EUR 400k-1.2M ideal for CFM: +2, else (1.2M-2.5M) +1.

### 2.6 Monte Carlo configuration

The deterministic Rust binary `footy-mc` (run via `scripts/run_cfm_mc.sh`) simulates 36 monthly steps with geometric-Brownian-style value paths:

- seed 42 everywhere; 1,000 paths per player; annual vol `sigma_ann = 0.4688` (the calibrated constant used by the 36m basket run);
- **Base run** (`artifacts/cfm_mc`): drift `mu_ann = ln(f36_predicted_12m_mv / current_mv)` (annual log drift from the v2 mean forecaster);
- **Spike run** (`artifacts/cfm_mc_spike`): drift `mu_ann = tail_tail_score` (the v3 expectile-0.9 12-month log-ratio IS the spike scenario);
- inputs are `data/cfm_mc_input.tsv` and `data/cfm_mc_input_spike.tsv` (11 columns each, written by `scripts/prep_cfm_mc.py`).

MC stats are terminal-36-month percentiles; mean/median/p10/p90 are in EUR (stats.tsv is post-processed from cents by the chart renderer), P(>=1.5x), P(>=2x), P(>=3x) and downside (P(< 0.5x)) are probabilities.

### 2.7 FBref and SofaScore join rules

- **SofaScore** (primary): search `api.sofascore.com/api/v1/search/all?q=<name>`; identity confirmed by normalized name + Transfermarkt DOB (19/20 exact DOB match; Leandersson matches by club + name without middle name, confidence medium). Season line chosen by documented rule: current season (2026 / 25/26 naming as of the June 2026 vintage), youth national-team competitions excluded, rated lines preferred, then most appearances (so loan and B-team players report the league where they actually play). Missing values stay blank; assists null in Serie C is reported as `missing`.
- **FBref** (secondary): blocked. Plain HTTP returns 403 and curl_cffi chrome impersonation also returns 403 (Cloudflare); the max-3-attempts rule was respected. All 20 players are flagged `fbref_unavailable = yes (Cloudflare 403)` and the per-90 columns are recorded as `unavailable`.

**Coverage counts:** SofaScore resolved 20/20 player ids, 20/20 with high/medium confidence, 20/20 season ratings; FBref 0/20 (blocked).

---

## 3. Alumni validation (the evidence base)

All 18 CF Montréal players who left for a fee in the TM transfers record, with their transfer outcome and success multiple (`current_mv / transfer_fee`):

| Player | Left (date) | To | Fee | Current value | Current club | Multiple |
|--------|------------|----|-----|---------------|--------------|----------|
| Maxime Crépeau | 2018-12-09 | Vancouver | EUR 45k | EUR 1.80M | Orlando City Soccer Club (MLS1) | 40.00x |
| Luis Binks | 2020-08-13 | Bologna | EUR 97k | EUR 3.50M | Bröndby IF (DK1) | 36.08x |
| Ariel Lassiter | 2024-08-14 | Chicago | EUR 68k | EUR 500k | Portland Timbers (MLS1) | 7.35x |
| Jonathan Sirois | 2026-02-02 | Dallas | EUR 127k | EUR 800k | FC Dallas (MLS1) | 6.30x |
| Joel Waterman | 2025-08-20 | Chicago | EUR 430k | EUR 2.00M | Chicago Fire Soccer Club (MLS1) | 4.65x |
| Chinonso Offor | 2024-07-01 | Arda Kardzhali | EUR 225k | EUR 1.00M | FK Baltika (RU1) | 4.44x |
| Nathan-Dylan Saliba | 2025-07-01 | RSC Anderlecht | EUR 2.00M | EUR 7.50M | RSC Anderlecht (BE1) | 3.75x |
| Amar Sejdić | 2021-07-07 | Atlanta | EUR 91k | EUR 300k | Columbus Crew (MLS1) | 3.30x |
| Aarón Herrera | 2023-12-12 | D.C. United | EUR 464k | EUR 1.50M | D.C. United (MLS1) | 3.23x |
| Ismaël Koné | 2023-01-01 | Watford | EUR 8.00M | EUR 25.00M | US Sassuolo (IT1) | 3.12x |
| Raheem Edwards | 2018-07-17 | Chicago | EUR 345k | EUR 800k | Toronto FC (MLS1) | 2.32x |
| Alistair Johnston | 2023-01-01 | Celtic | EUR 3.50M | EUR 8.00M | Celtic FC (SC1) | 2.29x |
| Djordje Mihailovic | 2023-01-01 | AZ Alkmaar | EUR 3.00M | EUR 5.50M | Toronto FC (MLS1) | 1.83x |
| Caden Clark | 2025-08-21 | D.C. United | EUR 600k | EUR 1.00M | D.C. United (MLS1) | 1.67x |
| George Campbell | 2025-07-18 | West Brom | EUR 1.30M | EUR 2.00M | West Bromwich Albion (GB1) | 1.54x |
| Dante Sealy | 2025-12-23 | Colorado | EUR 1.60M | EUR 1.50M | Colorado Rapids (MLS1) | 0.94x |
| Mathieu Choinière | 2024-08-27 | Grasshoppers | EUR 900k | EUR 800k | Los Angeles Football Club (MLS1) | 0.89x |
| Rudy Camacho | 2023-07-31 | Columbus | EUR 400k | EUR 150k | Columbus Crew (MLS1) | 0.38x |

Nine alumni meet the `success >= 1.5` and `fee > EUR 300k` bar; the top half by success (Waterman 4.65x, Saliba 3.75x, Herrera 3.23x, Koné 3.12x, Edwards 2.32x) defines the psych archetype in 2.2. The pattern is the classic bridge-club loop: sign young, develop, sell at a multiple (Crépeau 40x, Binks 36x on tiny fees; Koné 3.1x from EUR 8M to EUR 25M at Watford/Sassuolo).

---

## 4. Squad gap analysis (from data/cfm_squad.tsv, 33 players)

| Sub-position | Count | Assessment |
|--------------|-------|------------|
| Left Winger | 0 | **CRITICAL GAP: no left winger in the squad** |
| Left-Back | 2 | Gap (both 26+; Petrasso 26.0, Pearce 28.2) |
| Centre-Back | 6 | Overstocked |
| Centre-Forward | 5 | Overstocked |
| Right-Back | 4 | Stocked |
| Right Winger | 4 | Stocked (Synchuk 19.9 is the only young one) |
| Attacking Midfield | 3 | Medium (Jaime 25.7, Carmona 23.3, Escobar 19.8) |
| Central Midfield | 3 | Medium |
| Defensive Midfield | 3 | Medium (Piette 31.6) |
| Goalkeeper | 3 | Low need (Gillier 22, Breza 28, Keita 16.9) |

Squad age profile: mean 24.2, range 16.9-32.8; total squad value EUR 37.65M. The structural reads: **LW is the #1 gap**, **LB is #2** (age profile is old at the position), and AM/CM/DM are medium-priority. The shortlist fills gaps in order: LW x3, LB x3, CM x3, DM x3, CB x3, AM x2, GK/CF/RW x1 each.

---

## 5. Model artifacts and seeds

- Seed 42: used for the Monte Carlo (`--seed 42`) and for z-score construction determinism; no other stochastic steps exist in the scoring path.
- `data/forecast/model.bin` (v2 mean forecaster) SHA-256: `1d023e96f7fc045f23b2de0842af90df65f0851235883a7111606a5cadd4596f` (unchanged).
- `data/tail/model_exp090_r1e6.bin` (v3 expectile-0.9): mtime 2026-08-10 10:56:55 -0400 (unchanged).
- `data/surge/model.bin` (surge classifier): mtime 2026-08-10 02:16:18 -0400 (unchanged).

---

## 6. Top-20 detail

### Lisandro Piñero (#1)

- **Club/league:** Club Atlético Banfield (torneo-apertura, Argentina), FW/Left Winger, age 18.9.
- **Value & FM:** EUR 400k | CA 117 / PA 133.
- **Psych profile:** fit 0.627 (top attrs: Consistency 13, Composure 12, WorkRate 12, Teamwork 12).
- **Yield/spike:** 12m yield 67.7% | tail score 1.12.
- **Squad gap filled:** LW (squad_fit 7).
- **Performance:** SofaScore 6.7 (Liga Profesional de Fútbol, Primera LPF 2026): 0G, 0A, 13 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 10.48M | p10 3.74M | p90 30.31M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 1.77M.
- **Rationale:** 18.9-year-old left winger out of Banfield with the strongest combined score; consistent and composed, fits the LW gap exactly and sits in the U22 window.

Fan charts: `artifacts/cfm_mc/charts/chart_player_01.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_01.png` (spike); overview: `chart_overview.png` in each directory.

---

### Noah Tolf (#2)

- **Club/league:** Idrottsföreningen Kamraterna Göteborg (allsvenskan, Sweden), MF/Left-Back, age 20.8.
- **Value & FM:** EUR 1.00M | CA 110 / PA 120.
- **Psych profile:** fit 0.664 (top attrs: NaturalFitness 14, Teamwork 13, Determination 12, Consistency 12).
- **Yield/spike:** 12m yield 55.9% | tail score 1.04.
- **Squad gap filled:** LB (squad_fit 7).
- **Performance:** SofaScore 6.75 (Allsvenskan, Allsvenskan 2026): 0G, 1A, 8 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 20.66M | p10 7.37M | p90 59.75M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 3.56M.
- **Rationale:** IFK Göteborg left-back, 29 Allsvenskan starts last season; high natural fitness and teamwork, fills the old LB spot at 20.8.

Fan charts: `artifacts/cfm_mc/charts/chart_player_02.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_02.png` (spike); overview: `chart_overview.png` in each directory.

---

### Yannick Geiger (#3)

- **Club/league:** Allmänna Idrottsklubben (allsvenskan, Sweden), MF/Central Midfield, age 19.0.
- **Value & FM:** EUR 600k | CA 98 / PA 115.
- **Psych profile:** fit 0.643 (top attrs: Determination 14, Composure 12, Versatility 12, NaturalFitness 12).
- **Yield/spike:** 12m yield 65.2% | tail score 1.08.
- **Squad gap filled:** CM (squad_fit 6).
- **Performance:** SofaScore 6.83 (Allsvenskan, Allsvenskan 2026): 1G, 0A, 6 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 14.09M | p10 5.03M | p90 40.75M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 2.54M.
- **Rationale:** AIK central midfielder, determination 14; cheap, high-yield, U22 CM depth for the bridge pipeline.

Fan charts: `artifacts/cfm_mc/charts/chart_player_03.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_03.png` (spike); overview: `chart_overview.png` in each directory.

---

### Jeremías Merlo (#4)

- **Club/league:** Club de Gimnasia y Esgrima La Plata (torneo-apertura, Argentina), MF/Left Winger, age 20.0.
- **Value & FM:** EUR 700k | CA 117 / PA 118.
- **Psych profile:** fit 0.672 (top attrs: Determination 12, WorkRate 12, Bravery 11, Teamwork 11).
- **Yield/spike:** 12m yield 50.1% | tail score 1.02.
- **Squad gap filled:** LW (squad_fit 7).
- **Performance:** SofaScore 6.47 (Liga Profesional de Fútbol, Primera LPF 2026): 0G, 0A, 7 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 13.87M | p10 4.95M | p90 40.12M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 2.23M.
- **Rationale:** Gimnasia left winger, balanced mental profile; second LW option, U22 initiative eligible.

Fan charts: `artifacts/cfm_mc/charts/chart_player_04.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_04.png` (spike); overview: `chart_overview.png` in each directory.

---

### Niko Sepic (#5)

- **Club/league:** NK Istra 1961 (supersport-hnl, Croatia), MF/Left-Back, age 20.3.
- **Value & FM:** EUR 400k | CA 91 / PA 101.
- **Psych profile:** fit 0.598 (top attrs: NaturalFitness 14, Determination 13, WorkRate 13, Teamwork 13).
- **Yield/spike:** 12m yield 61.1% | tail score 1.06.
- **Squad gap filled:** LB (squad_fit 7).
- **Performance:** SofaScore 6.68 (HNL, HNL 25/26): 0G, 0A, 13 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 8.91M | p10 3.18M | p90 25.77M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 1.57M.
- **Rationale:** Istra 1961 left-back, 13 HNL starts at 20.3; high work rate and fitness, second LB option.

Fan charts: `artifacts/cfm_mc/charts/chart_player_05.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_05.png` (spike); overview: `chart_overview.png` in each directory.

---

### George Earthy (#6)

- **Club/league:** West Ham United (premier-league, England), MF/Attacking Midfield, age 21.8.
- **Value & FM:** EUR 1.00M | CA 112 / PA 116.
- **Psych profile:** fit 0.641 (top attrs: Versatility 14, Determination 13, Composure 13, WorkRate 13).
- **Yield/spike:** 12m yield 64.4% | tail score 1.08.
- **Squad gap filled:** AM (squad_fit 6).
- **Performance:** SofaScore 6.6 (Championship, Championship 25/26): 0G, 0A, 10 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 23.17M | p10 8.27M | p90 67.03M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 4.17M.
- **Rationale:** West Ham academy product (on loan, 10 Championship apps); versatile AM with good composure; the AM/10 slot behind the forwards.

Fan charts: `artifacts/cfm_mc/charts/chart_player_06.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_06.png` (spike); overview: `chart_overview.png` in each directory.

---

### Alfie Dorrington (#7)

- **Club/league:** Aberdeen FC (scottish-premiership, Scotland), MF/Centre-Back, age 21.1.
- **Value & FM:** EUR 500k | CA 102 / PA 124.
- **Psych profile:** fit 0.646 (top attrs: Determination 15, Composure 13, WorkRate 13, NaturalFitness 13).
- **Yield/spike:** 12m yield 68.0% | tail score 1.12.
- **Squad gap filled:** CB (squad_fit 5).
- **Performance:** SofaScore 6.47 (League Two, League Two 25/26): 0G, 1A, 10 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 13.15M | p10 4.69M | p90 38.03M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 2.22M.
- **Rationale:** Tottenham loanee at Aberdeen, CB with determination 15; premium long-term CB asset at EUR 500k.

Fan charts: `artifacts/cfm_mc/charts/chart_player_07.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_07.png` (spike); overview: `chart_overview.png` in each directory.

---

### Romeo Arrhenius Leandersson (#8)

- **Club/league:** Mjällby Almänna Idrottsförening (allsvenskan, Sweden), MF/Central Midfield, age 17.9.
- **Value & FM:** EUR 600k | CA 90 / PA 100.
- **Psych profile:** fit 0.624 (top attrs: Determination 13, Teamwork 13, Bravery 12, WorkRate 12).
- **Yield/spike:** 12m yield 62.2% | tail score 1.08.
- **Squad gap filled:** CM (squad_fit 6).
- **Performance:** SofaScore 6.61 (Allsvenskan, Allsvenskan 2026): 0G, 0A, 11 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 14.04M | p10 5.01M | p90 40.60M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 2.40M.
- **Rationale:** 17.9-year-old Mjällby central midfielder; youngest CM in the list, fits the U22 window perfectly.

Fan charts: `artifacts/cfm_mc/charts/chart_player_08.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_08.png` (spike); overview: `chart_overview.png` in each directory.

---

### Alessandro Romano (#9)

- **Club/league:** Associazione Sportiva Roma (serie-a, Italy), MF/Defensive Midfield, age 20.0.
- **Value & FM:** EUR 500k | CA 94 / PA 125.
- **Psych profile:** fit 0.597 (top attrs: ImportantMatches 14, Consistency 13, Versatility 13, Composure 12).
- **Yield/spike:** 12m yield 64.5% | tail score 1.10.
- **Squad gap filled:** DM (squad_fit 6).
- **Performance:** SofaScore 6.88 (Serie B, Serie B 25/26): 0G, 1A, 19 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 12.45M | p10 4.44M | p90 36.01M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 2.09M.
- **Rationale:** Roma youth product on loan at Spezia (19 Serie B apps); high ImportantMatches, DM depth at 19.9.

Fan charts: `artifacts/cfm_mc/charts/chart_player_09.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_09.png` (spike); overview: `chart_overview.png` in each directory.

---

### Andrea Natali (#10)

- **Club/league:** AZ Alkmaar (eredivisie, Netherlands), DF/Centre-Back, age 18.4.
- **Value & FM:** EUR 400k | CA 95 / PA -7.
- **Psych profile:** fit 0.619 (top attrs: Determination 15, ImportantMatches 14, NaturalFitness 14, Bravery 12).
- **Yield/spike:** 12m yield 68.4% | tail score 1.13.
- **Squad gap filled:** CB (squad_fit 5).
- **Performance:** SofaScore 6.76 (Eerste Divisie, Eerste Divisie 25/26): 0G, 1A, 11 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 10.71M | p10 3.82M | p90 30.97M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 100% | downside 0%. Base run (forecast drift) median 1.79M.
- **Rationale:** AZ's 18.4-year-old CB, 11 Eerste Divisie starts; determination 15, archetype-adjacent, cheap CB option.

Fan charts: `artifacts/cfm_mc/charts/chart_player_10.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_10.png` (spike); overview: `chart_overview.png` in each directory.

---

### Tiago Freitas (#11)

- **Club/league:** SL Benfica (liga-portugal, Portugal), MF/Central Midfield, age 19.8.
- **Value & FM:** EUR 550k | CA 102 / PA 107.
- **Psych profile:** fit 0.602 (top attrs: NaturalFitness 15, Determination 14, Consistency 14, Bravery 12).
- **Yield/spike:** 12m yield 62.6% | tail score 1.08.
- **Squad gap filled:** CM (squad_fit 6).
- **Performance:** SofaScore 6.62 (Liga Portugal 2, Liga Portugal 2 25/26): 3G, 2A, 22 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 13.03M | p10 4.65M | p90 37.69M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 2.22M.
- **Rationale:** Benfica B central midfielder, 22 Liga Portugal 2 apps; high natural fitness and consistency.

Fan charts: `artifacts/cfm_mc/charts/chart_player_11.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_11.png` (spike); overview: `chart_overview.png` in each directory.

---

### Felipe Bussio (#12)

- **Club/league:** Club Atlético Platense (torneo-apertura, Argentina), MF/Defensive Midfield, age 22.0.
- **Value & FM:** EUR 400k | CA 110 / PA 120.
- **Psych profile:** fit 0.644 (top attrs: Teamwork 14, WorkRate 13, Bravery 11, Composure 11).
- **Yield/spike:** 12m yield 57.3% | tail score 1.05.
- **Squad gap filled:** DM (squad_fit 6).
- **Performance:** SofaScore 6.6 (Liga Profesional de Fútbol, Primera LPF 2026): 0G, 0A, 11 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 8.48M | p10 3.03M | p90 24.54M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 1.46M.
- **Rationale:** Platense DM, 11 LPF starts; team-first profile (teamwork 14) for the holding slot.

Fan charts: `artifacts/cfm_mc/charts/chart_player_12.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_12.png` (spike); overview: `chart_overview.png` in each directory.

---

### Andrea Cogoni (#13)

- **Club/league:** Cagliari Calcio (serie-a, Italy), MF/Centre-Back, age 19.9.
- **Value & FM:** EUR 400k | CA 89 / PA 92.
- **Psych profile:** fit 0.616 (top attrs: ImportantMatches 15, Determination 12, Bravery 12, Composure 11).
- **Yield/spike:** 12m yield 62.0% | tail score 1.08.
- **Squad gap filled:** CB (squad_fit 5).
- **Performance:** SofaScore 7.02 (Campionato Primavera 1, Primavera 1 25/26): 2G, 2A, 30 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 9.44M | p10 3.37M | p90 27.31M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 1.60M.
- **Rationale:** Cagliari youth CB, 30 Primavera apps with a 7.02 rating; ImportantMatches 15.

Fan charts: `artifacts/cfm_mc/charts/chart_player_13.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_13.png` (spike); overview: `chart_overview.png` in each directory.

---

### Wassim Bouziane (#14)

- **Club/league:** AZ Alkmaar (eredivisie, Netherlands), FW/Left Winger, age 19.5.
- **Value & FM:** EUR 450k | CA 112 / PA 128.
- **Psych profile:** fit 0.602 (top attrs: Determination 14, ImportantMatches 13, NaturalFitness 13, Composure 12).
- **Yield/spike:** 12m yield 49.0% | tail score 0.98.
- **Squad gap filled:** LW (squad_fit 7).
- **Performance:** SofaScore 7.08 (Eerste Divisie, Eerste Divisie 25/26): 4G, 3A, 20 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 7.73M | p10 2.76M | p90 22.35M | P(>=1.5x) 100% | P(>=2x) 99% | P(>=3x) 98% | downside 0%. Base run (forecast drift) median 1.40M.
- **Rationale:** AZ left winger, 20 Eerste Divisie apps with 4 goals and 3 assists (7.08 rating); direct LW cover.

Fan charts: `artifacts/cfm_mc/charts/chart_player_14.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_14.png` (spike); overview: `chart_overview.png` in each directory.

---

### Diogo Ferreira (#15)

- **Club/league:** SL Benfica (liga-portugal, Portugal), GK/Goalkeeper, age 19.3.
- **Value & FM:** EUR 800k | CA 103 / PA 108.
- **Psych profile:** fit 0.623 (top attrs: Determination 14, NaturalFitness 14, Bravery 12, Consistency 12).
- **Yield/spike:** 12m yield 59.0% | tail score 1.07.
- **Squad gap filled:** GK (squad_fit 5).
- **Performance:** SofaScore 6.85 (Liga Portugal 2, Liga Portugal 2 25/26): 0G, 0A, 26 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 17.99M | p10 6.42M | p90 52.04M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 3.02M.
- **Rationale:** Benfica B goalkeeper, 26 Liga Portugal 2 apps at 6.85; long-term GK succession behind Gillier.

Fan charts: `artifacts/cfm_mc/charts/chart_player_15.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_15.png` (spike); overview: `chart_overview.png` in each directory.

---

### Olivier Mambwa (#16)

- **Club/league:** BSC Young Boys (super-league, Switzerland), DF/Left-Back, age 17.5.
- **Value & FM:** EUR 1.00M | CA 96 / PA 105.
- **Psych profile:** fit 0.576 (top attrs: NaturalFitness 15, Determination 13, WorkRate 12, Teamwork 12).
- **Yield/spike:** 12m yield 50.0% | tail score 1.00.
- **Squad gap filled:** LB (squad_fit 7).
- **Performance:** SofaScore 6.14 (Swiss Super League, Super League 25/26): 0G, 1A, 7 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 18.25M | p10 6.51M | p90 52.80M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 98% | downside 0%. Base run (forecast drift) median 3.18M.
- **Rationale:** 17.5-year-old Young Boys U21 left-back already with 7 Super League apps; the youngest LB prospect.

Fan charts: `artifacts/cfm_mc/charts/chart_player_16.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_16.png` (spike); overview: `chart_overview.png` in each directory.

---

### Matija Subotic (#17)

- **Club/league:** NK Lokomotiva Zagreb (supersport-hnl, Croatia), MF/Defensive Midfield, age 18.5.
- **Value & FM:** EUR 850k | CA 96 / PA 96.
- **Psych profile:** fit 0.599 (top attrs: Versatility 14, Composure 13, Teamwork 13, NaturalFitness 13).
- **Yield/spike:** 12m yield 55.7% | tail score 1.02.
- **Squad gap filled:** DM (squad_fit 6).
- **Performance:** SofaScore 6.67 (HNL, HNL 25/26): 0G, 2A, 32 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 16.69M | p10 5.95M | p90 48.28M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 3.02M.
- **Rationale:** Lokomotiva Zagreb DM, 32 HNL apps; versatile with high composure, DM depth.

Fan charts: `artifacts/cfm_mc/charts/chart_player_17.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_17.png` (spike); overview: `chart_overview.png` in each directory.

---

### Matteo Spinaccè (#18)

- **Club/league:** Inter Milan (serie-a, Italy), FW/Centre-Forward, age 19.9.
- **Value & FM:** EUR 1.00M | CA 96 / PA 132.
- **Psych profile:** fit 0.628 (top attrs: Composure 13, WorkRate 13, Teamwork 13, Consistency 12).
- **Yield/spike:** 12m yield 58.2% | tail score 1.05.
- **Squad gap filled:** CF (squad_fit 5).
- **Performance:** SofaScore 6.74 (Serie C, Girone A, Serie C, Group A 25/26): 6G, A, 26 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 21.18M | p10 7.56M | p90 61.28M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 3.72M.
- **Rationale:** Inter youth striker on loan at Mantova, 6 goals in 26 Serie C starts; U22 CF option despite CF stock.

Fan charts: `artifacts/cfm_mc/charts/chart_player_18.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_18.png` (spike); overview: `chart_overview.png` in each directory.

---

### Adrian Kurd Rønning (#19)

- **Club/league:** Kristiansund Ballklubb (eliteserien, Norway), FW/Attacking Midfield, age 20.2.
- **Value & FM:** EUR 450k | CA 94 / PA 110.
- **Psych profile:** fit 0.704 (top attrs: WorkRate 14, Bravery 13, NaturalFitness 13, Determination 12).
- **Yield/spike:** 12m yield 43.2% | tail score 0.93.
- **Squad gap filled:** AM (squad_fit 6).
- **Performance:** SofaScore 6.54 (Eliteserien, Eliteserien 2026): 1G, 0A, 16 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 6.76M | p10 2.41M | p90 19.56M | P(>=1.5x) 100% | P(>=2x) 99% | P(>=3x) 97% | downside 0%. Base run (forecast drift) median 1.25M.
- **Rationale:** Kristiansund attacking midfielder, 16 Eliteserien apps; highest psych_fit in the list (0.704).

Fan charts: `artifacts/cfm_mc/charts/chart_player_19.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_19.png` (spike); overview: `chart_overview.png` in each directory.

---

### Seydou Fini (#20)

- **Club/league:** Genoa CFC (serie-a, Italy), FW/Right Winger, age 20.0.
- **Value & FM:** EUR 1.00M | CA 112 / PA 135.
- **Psych profile:** fit 0.655 (top attrs: NaturalFitness 16, Consistency 13, WorkRate 12, Teamwork 12).
- **Yield/spike:** 12m yield 53.2% | tail score 1.01.
- **Squad gap filled:** RW (squad_fit 5).
- **Performance:** SofaScore 7.06 (Serie B, Serie B 25/26): 1G, 1A, 8 apps. FBref: unavailable (Cloudflare 403).
- **Monte Carlo (spike run, seed 42):** median 19.16M | p10 6.84M | p90 55.44M | P(>=1.5x) 100% | P(>=2x) 100% | P(>=3x) 99% | downside 0%. Base run (forecast drift) median 3.38M.
- **Rationale:** Genoa winger on loan at Frosinone, 1 goal and 1 assist in 8 Serie B apps at 7.06 rating; RW succession.

Fan charts: `artifacts/cfm_mc/charts/chart_player_20.png` (base) and `artifacts/cfm_mc_spike/charts/chart_player_20.png` (spike); overview: `chart_overview.png` in each directory.

---

## 7. Honest limitations

- **Hidden personality coverage is partial (~32% of the panel; 0 values are FM placeholders and treated as missing).** The archetype's hidden component therefore relies on the documented bridge-club prior, not on alumni hidden data. All top-20 players do carry real values for the four hidden attrs used (Pressure, Professionalism, Temperament, Controversy) where present.
- **The alumni calibration set is small (5 players).** It is the best available evidence for a club of this size, but psych-fit differences of a few hundredths should not be over-read.
- **FBref coverage is 0/20** (Cloudflare 403; max-attempts rule respected). SofaScore covers 20/20 with ratings but the underlying season lines vary in competition level (youth leagues, loan clubs, second tiers), which is stated per player.
- **Data vintage mismatch:** FM26 attributes reflect approximately November 2025; TM valuations run through 2026-06-12; SofaScore statistics are live. Loan moves after the FM/TM vintage (Spezia, Mantova, Frosinone, Championship, League Two) are visible in SofaScore but not in the TM transfers table.
- **Tail scores are model outputs, not guarantees.** The expectile-0.9 drift is the spike scenario: P(>=1.5x) near 100% under that regime describes what happens *if* the spike materializes, not a probability-weighted forecast. Base-run medians are the conservative comparison.
- **Single-window model scores are not guarantees** of future transfer outcomes; they rank candidates within this basket and vintage only.

---

*Generated by the CFM scouting pipeline (STEERING_CFM.md). Seed 42; models unchanged (hashes in section 5).*
