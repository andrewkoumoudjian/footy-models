#!/usr/bin/env python3
"""SofaScore enrichment for the CFM scout top-20 (STEERING_CFM.md section 6).

Improved v2:
  * search fallback: full name, then name without middle names, then last name
  * identity: name + DOB match (TM DOB from the shortlist panel join)
  * season choice: query the top-3 candidate (tournament, season) pairs ordered by
    (tournament-match score vs TM competition / profile team league, recency);
    pick the one with the MOST appearances (loan/youth players report their real
    playing environment). Missing = blank, never zero.
"""
import json
import re
import sys
import time
import unicodedata

import pandas as pd
from curl_cffi import requests as creq

HEADERS = {
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Accept": "application/json",
    "Accept-Language": "en-US,en;q=0.9",
    "Referer": "https://www.sofascore.com/",
}
BASE = "https://api.sofascore.com/api/v1"


def norm(s):
    s = unicodedata.normalize("NFKD", str(s))
    s = "".join(c for c in s if not unicodedata.combining(c))
    return re.sub(r"\s+", " ", re.sub(r"[^a-z0-9]+", " ", s.lower())).strip()


def tokens(s):
    return set(norm(s).split())


def get(path, tries=2):
    for i in range(tries):
        try:
            r = creq.get(BASE + path, headers=HEADERS, impersonate="chrome", timeout=25)
            if r.status_code == 200:
                return r.json()
            if r.status_code in (404, 400):
                return None
        except Exception as e:
            print(f"  retry {i+1} {path}: {e}", file=sys.stderr)
        time.sleep(1.2)
    return None


def search_candidates(name):
    queries = [name]
    parts = norm(name).split()
    if len(parts) >= 3:
        queries.append(" ".join(parts[0:1] + parts[2:]))       # drop middle
        queries.append(parts[0])                                 # first
        queries.append(parts[-1])                                # last
    seen = set()
    out = []
    for q in queries:
        if q in seen:
            continue
        seen.add(q)
        j = get("/search/all?q=" + q.replace(" ", "%20"))
        if not j:
            continue
        for x in j.get("results", []):
            e = x.get("entity", {})
            if x.get("type") != "player":
                continue
            pid = e.get("id")
            if pid in {c[0] for c in out}:
                continue
            out.append((pid, e.get("name", ""), ((e.get("team") or {}).get("name") or ""),
                        (e.get("dateOfBirth") or "")[:10]))
        if out:
            break
    return out


def tourn_match_score(ut_slug, ut_name, tm_comp, prof_tourn):
    """Score a SofaScore tournament against the TM competition slug and the
    profile team's tournament slug. Generic single tokens ('league','cup','1')
    never score."""
    GENERIC = {"league", "cup", "1", "2", "u21", "u19", "qualification"}
    tc = norm(tm_comp)
    prof = norm(prof_tourn or "")
    ns = norm(ut_slug) + " " + norm(ut_name)
    score = 0
    for key, w in ((tc, 3), (prof, 3)):
        if not key:
            continue
        if key and key in ns:
            score = max(score, w)
            continue
        for tok in key.split():
            if len(tok) >= 5 and tok in ns and tok not in GENERIC:
                score = max(score, w - 1)
    return score


YOUTH_PAT = ("u15", "u16", "u17", "u18", "u19", "u20", "u21", "u23",
                 "youth league", "juniori", "qualification", "world championship",
                 "european championship", "elite league", "national league cup")


def season_rank(se_name):
    """Current season at the June-2026 data vintage: '2026' or '25/26' names are
    current (1); '2025'/'24/25' are previous (2); anything else (e.g. upcoming
    '26/27') is not considered evidence (3)."""
    n = norm(se_name)  # norm() turns '25/26' into '25 26' (slash stripped)
    if "2026" in n or "25 26" in n:
        return 1
    if "2025" in n or "24 25" in n:
        return 2
    return 3


def best_season(pid, tm_comp, prof_tourn):
    j = get(f"/player/{pid}/statistics/seasons")
    if not j:
        return None
    entries = []
    for s in j.get("uniqueTournamentSeasons", []):
        ut = s.get("uniqueTournament", {})
        for se in s.get("seasons", []):
            entries.append((ut.get("id"), ut.get("name", ""), ut.get("slug", ""),
                            se.get("id"), se.get("name", ""), str(se.get("year", ""))))
    if not entries:
        return None
    kept = []
    for e in entries:
        ut_id, ut_name, ut_slug, se_id, se_name, se_year = e
        nname = norm(ut_name) + " " + norm(ut_slug)
        if any(p in nname for p in YOUTH_PAT):
            continue  # youth national-team / youth-league comps are not the scout line
        kept.append((season_rank(se_name), e))
    if not kept:
        return None
    kept.sort(key=lambda x: (x[0], tourn_match_score(x[1][2], x[1][1], tm_comp, prof_tourn),
                             x[1][3]))
    return [e for _, e in kept[:6]]


def season_stats(pid, ut_id, se_id):
    """Overall stats for a season line; retries empty responses (SofaScore
    intermittently returns statistics:null under rapid-fire)."""
    for _ in range(4):
        j = get(f"/player/{pid}/unique-tournament/{ut_id}/season/{se_id}/statistics/overall")
        st = (j or {}).get("statistics") or None
        if st is not None:
            return st
        time.sleep(2.0)
    return None


def main():
    top = pd.read_csv("/home/drewclaw/footy-models/data/cfm_top20_shortlist.csv")
    # TM DOB map (verified against the read-only TM oracle) for identity checks
    import duckdb
    con = duckdb.connect('/home/drewclaw/transfermarkt/transfermarkt-datasets.duckdb', read_only=True)
    tm_dob = {int(r[0]): (r[1].strftime('%Y-%m-%d') if r[1] else '')
              for r in con.execute("SELECT player_id, date_of_birth FROM players").fetchall()}
    con.close()
    out_rows = []
    for _, r in top.iterrows():
        name, club, comp = r["name"], r["club"], r["competition"]
        print(f"--- {name} ({club})")
        row = {"player_id": r["player_id"], "name": name,
               "sofascore_id": "", "sofascore_confidence": "", "sofascore_team": "",
               "sofascore_position": "", "sofascore_dob": "", "sofascore_dob_match": "",
               "sofascore_tournament": "", "sofascore_season": "",
               "sofascore_rating": "", "sofascore_goals": "", "sofascore_assists": "",
               "sofascore_appearances": ""}
        cands = search_candidates(name)
        if not cands:
            print("  no candidates")
            out_rows.append(row)
            continue
        best = None
        for pid, pname, pteam, pdob in cands:
            # prefer exact name, then name+team/DOB consistency
            sc = 0
            if norm(pname) == norm(name):
                sc += 2
            elif norm(name).startswith(norm(pname)) or norm(pname).startswith(norm(name)):
                sc += 1
            if tokens(pteam) & tokens(club):
                sc += 1
            if pdob:
                sc += 0.5
            if best is None or sc > best[0]:
                best = (sc, pid, pname, pteam, pdob)
        _, pid, pname, pteam, pdob = best
        time.sleep(0.6)
        prof = get(f"/player/{pid}") or {}
        p = prof.get("player", {})
        pteam_prof = (p.get("team") or {}).get("name", "")
        pdob_prof = (p.get("dateOfBirth") or "")[:10]
        prof_tourn = ((p.get("team") or {}).get("tournament") or {}).get("name", "")
        team = pteam_prof or pteam
        dob = pdob_prof or pdob
        tm_dob_ok = bool(tm_dob.get(int(r["player_id"])) and tm_dob.get(int(r["player_id"])) == dob)
        # confidence: exact name + (dob or club) -> high; club/dob alone -> medium; else low
        conf = "low"
        if norm(pname) == norm(name) or (tokens(team) & tokens(club)) or (dob and tm_dob_ok):
            conf = "medium"
        if conf == "medium" and norm(pname) == norm(name) and ((tokens(team) & tokens(club)) or tm_dob_ok):
            conf = "high"
        row.update(sofascore_id=pid, sofascore_confidence=conf, sofascore_team=team,
                   sofascore_position=p.get("position", ""), sofascore_dob=dob,
                   sofascore_dob_match=("yes" if tm_dob_ok else ""))
        print(f"  match {pid} {pname} @ {team} conf={conf}")
        time.sleep(0.6)
        cands3 = best_season(pid, comp, prof_tourn)
        if cands3:
            picked = None
            for (ut_id, ut_name, ut_slug, se_id, se_name, se_year) in cands3:
                time.sleep(0.8)
                st = season_stats(pid, ut_id, se_id)
                if not st:
                    continue
                apps = st.get("appearances") or 0
                rated = st.get("rating") is not None
                srank = season_rank(se_name)
                # current season first: negate srank so lower rank number wins
                key = (-srank, 1 if rated else 0, apps, tourn_match_score(ut_slug, ut_name, comp, prof_tourn))
                if picked is None or key > picked[0]:
                    picked = (key, ut_name, se_name, st)
            if picked:
                _, ut_name, se_name, st = picked
                row["sofascore_tournament"] = ut_name
                row["sofascore_season"] = se_name
                row["sofascore_rating"] = st.get("rating")
                row["sofascore_goals"] = st.get("goals")
                row["sofascore_assists"] = st.get("assists")
                row["sofascore_appearances"] = st.get("appearances")
        out_rows.append(row)
        print("  ->", {k: v for k, v in row.items() if k != "name"})
        time.sleep(0.6)

    out = pd.DataFrame(out_rows)
    out.to_csv("/home/drewclaw/footy-models/data/cfm_sofascore_enrich.csv", index=False)
    n_conf = out["sofascore_confidence"].isin(["high", "medium"]).sum()
    print(f"\nresolved: {out['sofascore_id'].astype(str).str.len().gt(0).sum()}/20 ids, "
          f"{n_conf}/20 high/medium, rating coverage {out['sofascore_rating'].notna().sum()}/20")


if __name__ == "__main__":
    main()
