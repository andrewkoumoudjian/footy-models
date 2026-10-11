#!/usr/bin/env node
/**
 * Generate the print-ready CFM scout report HTML per pdf-design skill.
 * Reuses footy-site/src/cfmData.ts data (single source of truth).
 * Fonts: SF Pro Text (body) + New York (major section titles), embedded locally.
 * Letter 8.5x11in, sentence case, left-aligned, tight tracking.
 */
const fs = require("fs");
const path = require("path");

// Load cfmData.ts by transpiling the export (it's TS with type annotation)
const src = fs.readFileSync("/home/drewclaw/footy-site/src/cfmData.ts", "utf8");
const stripped = src
  .replace(/export type CFMPlayer = \{[\s\S]*?\};/, "")
  .replace(/export const CFM_PLAYERS: CFMPlayer\[\] =/, "const CFM_PLAYERS =")
  .replace(/\brank: /g, "rank: ")
  .trim();
const captured = {};
eval(stripped + "\ncaptured.CFM_PLAYERS = CFM_PLAYERS;");
const P = captured.CFM_PLAYERS;

const fmtEur = (v) => (v >= 1e6 ? "€" + (v / 1e6).toFixed(2) + "M" : v >= 1e3 ? "€" + Math.round(v / 1e3) + "k" : "€" + Math.round(v));
const pct = (v) => (v * 100).toFixed(0) + "%";
const fp = (v) => (v >= 0.995 ? ">99%" : pct(v));

const ALUMNI = [
  ["Joel Waterman", "€430k", "€2.00M", "4.65x"], ["Nathan-Dylan Saliba", "€2.00M", "€7.50M", "3.75x"],
  ["Aarón Herrera", "€464k", "€1.50M", "3.23x"], ["Ismaël Koné", "€8.00M", "€25.00M", "3.12x"],
  ["Raheem Edwards", "€345k", "€800k", "2.32x"],
];
const ALUMNI_ALL = [
  ["Maxime Crépeau", "2018", "Vancouver", "€45k", "€1.80M", "Orlando City", "40.00x"],
  ["Luis Binks", "2020", "Bologna", "€97k", "€3.50M", "Bröndby", "36.08x"],
  ["Ariel Lassiter", "2024", "Chicago", "€68k", "€500k", "Portland", "7.35x"],
  ["Jonathan Sirois", "2026", "Dallas", "€127k", "€800k", "FC Dallas", "6.30x"],
  ["Joel Waterman", "2025", "Chicago", "€430k", "€2.00M", "Chicago Fire", "4.65x"],
  ["Chinonso Offor", "2024", "Arda Kardzhali", "€225k", "€1.00M", "FK Baltika", "4.44x"],
  ["Nathan-Dylan Saliba", "2025", "Anderlecht", "€2.00M", "€7.50M", "Anderlecht", "3.75x"],
  ["Amar Sejdić", "2021", "Atlanta", "€91k", "€300k", "Columbus", "3.30x"],
  ["Aarón Herrera", "2023", "D.C. United", "€464k", "€1.50M", "D.C. United", "3.23x"],
  ["Ismaël Koné", "2023", "Watford", "€8.00M", "€25.00M", "Sassuolo", "3.12x"],
  ["Raheem Edwards", "2018", "Chicago", "€345k", "€800k", "Toronto FC", "2.32x"],
  ["Alistair Johnston", "2023", "Celtic", "€3.50M", "€8.00M", "Celtic", "2.29x"],
  ["Djordje Mihailovic", "2023", "AZ Alkmaar", "€3.00M", "€5.50M", "Toronto FC", "1.83x"],
  ["Caden Clark", "2025", "D.C. United", "€600k", "€1.00M", "D.C. United", "1.67x"],
  ["George Campbell", "2025", "West Brom", "€1.30M", "€2.00M", "West Brom", "1.54x"],
  ["Dante Sealy", "2025", "Colorado", "€1.60M", "€1.50M", "Colorado", "0.94x"],
  ["Mathieu Choinière", "2024", "Grasshoppers", "€900k", "€800k", "LAFC", "0.89x"],
  ["Rudy Camacho", "2023", "Columbus", "€400k", "€150k", "Columbus", "0.38x"],
];
const ATTRS = [
  ["Determination", "11.25"], ["Bravery", "11.20"], ["Consistency", "11.80"], ["Important matches", "8.75"],
  ["Composure", "12.00"], ["Work rate", "12.40"], ["Teamwork", "12.00"], ["Leadership", "10.40"],
  ["Versatility", "10.80"], ["Natural fitness", "12.75"], ["Injury proneness", "10.33"],
];
const GAPS = [
  ["Left winger", "0", "Critical gap — no left winger in the squad"],
  ["Left-back", "2", "Gap — both 26+ (Petrasso 26.0, Pearce 28.2)"],
  ["Centre-back", "6", "Overstocked"], ["Centre-forward", "5", "Overstocked"],
  ["Right-back", "4", "Stocked"], ["Right winger", "4", "Stocked — Synchuk 19.9 only young option"],
  ["Attacking midfield", "3", "Medium — Jaime 25.7, Carmona 23.3, Escobar 19.8"],
  ["Central midfield", "3", "Medium"], ["Defensive midfield", "3", "Medium — Piette 31.6"],
  ["Goalkeeper", "3", "Low need — Gillier 22, Breza 28, Keita 16.9"],
];

const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

// ---- HTML helpers (pdf-design skill conventions) ----
const statsStrip = (items) => `<div class="stats-strip">${items.map(i => `<div class="stat"><div class="big">${i.big}</div><div class="label">${i.label}</div></div>`).join("")}</div>`;

const CLUB_SHORT = {
  "Club Atlético Banfield": "Banfield",
  "Idrottsföreningen Kamraterna Göteborg": "IFK Göteborg",
  "Allmänna Idrottsklubben": "AIK",
  "Club de Gimnasia y Esgrima La Plata": "Gimnasia LP",
  "NK Istra 1961": "Istra 1961",
  "West Ham United": "West Ham",
  "Aberdeen FC": "Aberdeen",
  "Mjällby Almänna Idrottsförening": "Mjällby",
  "Associazione Sportiva Roma": "Roma",
  "AZ Alkmaar": "AZ Alkmaar",
  "SL Benfica": "Benfica",
  "Club Atlético Platense": "Platense",
  "Cagliari Calcio": "Cagliari",
  "BSC Young Boys": "Young Boys",
  "NK Lokomotiva Zagreb": "Lokomotiva",
  "Inter Milan": "Inter",
  "Kristiansund Ballklubb": "Kristiansund",
  "Genoa CFC": "Genoa",
};

const execRows = P.map(p => `<tr><td class="c">${p.rank}</td><td class="n">${esc(p.name)}<span class="gap-tag">${p.squadGap}</span></td><td>${esc(CLUB_SHORT[p.club] || p.club)}</td><td>${p.pos}</td><td class="r">${p.age.toFixed(1)}</td><td class="r">${fmtEur(p.mv)}</td><td class="r">${p.psychFit.toFixed(3)}</td><td class="r strong">${p.tail.toFixed(2)}</td><td class="r pos">${pct(p.yield12m / 100)}</td><td class="r">${p.sRating}</td><td class="r">${fmtEur(p.mcMedian)}</td><td class="r pos">${fp(p.p15x)}</td></tr>`).join("");

const playerCards = P.map(p => `
<div class="player-card">
  <div class="pc-head">
    <div class="pc-id"><span class="pc-rank">${String(p.rank).padStart(2, "0")}</span>
      <div><div class="pc-name">${esc(p.name)}</div>
      <div class="pc-sub">${esc(p.club)} · ${esc(p.league)} (${esc(p.country)}) · age ${p.age.toFixed(1)} · ${p.pos}${p.squadGap ? ` · <span class="gap-tag">fills ${p.squadGap}</span>` : ""}</div></div>
    </div>
    <div class="pc-meta"><span>${fmtEur(p.mv)}</span><span class="dim">CA ${p.ca} / PA ${p.pa}</span><span class="dim">psych ${p.psychFit.toFixed(3)} · tail ${p.tail.toFixed(2)} · 12m ${pct(p.yield12m / 100)}</span></div>
  </div>
  <div class="pc-stats">
    <div class="pcs"><div class="pcs-label">Psych fit</div><div class="pcs-big">${p.psychFit.toFixed(3)}</div><div class="pcs-sub">${p.topPsych.join(" · ")}</div></div>
    <div class="pcs"><div class="pcs-label">SofaScore</div><div class="pcs-big">${p.sRating}</div><div class="pcs-sub">${p.sGoals}G ${p.sAssists}A · ${p.sApps} apps</div></div>
    <div class="pcs"><div class="pcs-label">MC median 36m</div><div class="pcs-big">${fmtEur(p.mcMedian)}</div><div class="pcs-sub">spike · base ${fmtEur(p.baseMedian)}</div></div>
    <div class="pcs"><div class="pcs-label">10–90% band</div><div class="pcs-big">${fmtEur(p.mcP10)}</div><div class="pcs-sub">to ${fmtEur(p.mcP90)}</div></div>
    <div class="pcs"><div class="pcs-label">P(≥1.5× / ≥2× / ≥3×)</div><div class="pcs-big">${fp(p.p15x)} / ${fp(p.p2x)} / ${fp(p.p3x)}</div><div class="pcs-sub">downside ${pct(p.downside)}</div></div>
  </div>
  <div class="pc-charts">
  <figure class="chart"><figcaption>Spike run — drift = v3 expectile-0.9 tail · seed 42 · 1,000 paths</figcaption><img src="cfm/charts/${p.chart.split("/").pop()}" alt="${esc(p.name)} spike fan chart"></figure>
  <figure class="chart"><figcaption>Base run — drift = v2 forecast · seed 42 · 1,000 paths</figcaption><img src="cfm/charts_base/${p.chartBase.split("/").pop()}" alt="${esc(p.name)} base fan chart"></figure>
  </div>
  <div class="pc-rationale"><div class="pcs-label">Squad fit + rationale</div><p>${esc(p.rationale)} <span class="dim">· squad_fit ${p.squadFit} (gap ${p.squadGap || "none"}) · ${esc(p.sTournament)}, ${esc(p.sSeason)} · confidence ${p.sConfidence} · FBref unavailable (Cloudflare 403)</span></p></div>
  </div>`).join("");

const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>CF Montréal scouting report — top-20 psych-fit, positive-yield value-spike targets</title>
<style>
  /* pdf-design skill: letter grid pages, sentence case, left-aligned, tight tracking */
  @page { size: letter; margin: 0; }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  :root {
    --civic-navy: #111111;
    --civic-blue: #3a3a3a;
    --civic-gold: #3a3a3a;
    --civic-red: #111111;
    --ink: #111111;
    --body: #3a3a3a;
    --line: #d5d5d5;
    --paper: #ffffff;
    --panel: #f6f6f6;
  }
  html { font-size: 10pt; }
  body { font-family: "SF Pro Text", "SF Pro Display", "Helvetica Neue", sans-serif; background: var(--paper); color: var(--ink); line-height: 1.5; }
  .page { width: 8.5in; height: 11in; background: var(--paper); display: grid; grid-template-rows: auto 1fr auto; overflow: hidden; page-break-after: always; }
  .page:last-child { page-break-after: auto; }
  /* Typography: New York regular for major section titles, SF Pro Text for body */
  h1, h2, h3, .cover-title, .section-title { font-family: "New York Medium", "New York Small", "New York", "Times New Roman", Georgia, serif; color: var(--ink); letter-spacing: -0.005em; font-weight: 400; }
  .cover-title { font-size: 40pt; font-weight: 400; line-height: 1.08; letter-spacing: -0.01em; }
  .section-title { font-size: 19pt; font-weight: 400; line-height: 1.15; letter-spacing: -0.005em; }
  .section-title::after { content: ''; display: block; width: 0.95in; height: 0.04in; background: var(--civic-red); margin-top: 0.12in; }
  p { letter-spacing: -0.005em; }
  h3 { font-size: 12pt; font-weight: 600; margin: 0.2in 0 0.08in; }
  .small { font-size: 8.5pt; letter-spacing: -0.005em; }
  .dim { color: var(--body); }
  .pos { color: var(--civic-blue); font-weight: 600; }
  .strong { font-weight: 700; }
  .r { text-align: right; }
  .c { text-align: center; }
  /* Cover */
  .cover { padding: 0.55in 0.7in; grid-template-rows: auto 1fr auto; }
  .cover-org { font-size: 8.5pt; font-weight: 600; letter-spacing: 0.14em; text-transform: uppercase; color: var(--civic-red); margin-bottom: 0.25in; }
  .cover-kicker { font-size: 10.5pt; font-weight: 600; color: var(--civic-blue); margin-top: 0.12in; letter-spacing: -0.005em; }
  .cover-intro { max-width: 6.2in; margin-top: 0.28in; font-size: 10pt; color: var(--body); line-height: 1.55; }
  .cover-footer { border-top: 1px solid var(--line); padding-top: 0.3in; }
  .cover-meta { display: grid; grid-template-columns: 1fr 1fr; gap: 0.2in; margin-bottom: 0.28in; }
  .meta-box { padding: 0.16in 0.2in; background: var(--panel); border-left: 3px solid var(--civic-blue); }
  .meta-box .lbl { font-size: 7.5pt; font-weight: 700; letter-spacing: 0.1em; text-transform: uppercase; color: var(--body); }
  .meta-box .val { font-size: 9pt; margin-top: 0.05in; letter-spacing: -0.005em; }
  /* Stats strip */
  .stats-strip { display: grid; grid-template-columns: repeat(4, 1fr); gap: 0.24in; margin-top: 0.3in; }
  .stat { padding: 0.05in 0 0.1in 0.2in; border-left: 4px solid var(--civic-red); }
  .stat .big { font-family: "SF Pro Display", sans-serif; font-size: 24pt; font-weight: 700; line-height: 1; color: var(--civic-navy); letter-spacing: -0.02em; }
  .stat .label { font-size: 8pt; font-weight: 600; margin-top: 0.09in; color: var(--body); text-transform: uppercase; letter-spacing: 0.06em; }
  /* Content pages */
  .page-body { padding: 0.45in 0.7in 0.4in; overflow: hidden; }
  .page-header { display: flex; justify-content: space-between; align-items: baseline; padding: 0.35in 0.7in 0.18in; border-bottom: 1px solid var(--line); }
  .page-header .doc { font-size: 8.5pt; font-weight: 600; color: var(--civic-navy); letter-spacing: 0.08em; text-transform: uppercase; }
  .page-header .pg { font-size: 8.5pt; color: var(--body); }
  .page-footer { padding: 0.12in 0.7in 0.3in; border-top: 1px solid var(--line); font-size: 7.5pt; color: var(--body); display: flex; justify-content: space-between; }
  .section { margin-bottom: 0.28in; }
  .section-title { margin-bottom: 0.16in; }
  h3 { font-size: 12pt; font-weight: 600; margin: 0.2in 0 0.08in; }
  h3.first { margin-top: 0; }
  .lede { margin-bottom: 0.14in; }
  .body-copy { margin-bottom: 0.12in; font-size: 9pt; }
  code, .mono { font-family: "SF Mono", "Menlo", monospace; font-size: 8.5pt; background: var(--panel); padding: 0.02in 0.06in; border-radius: 2px; }
  pre.formula { font-family: "SF Mono", monospace; font-size: 8.5pt; background: var(--panel); border-left: 3px solid var(--civic-blue); padding: 0.12in 0.16in; margin: 0.1in 0 0.14in; white-space: pre-wrap; letter-spacing: -0.005em; }
  /* Tables */
  table { width: 100%; border-collapse: collapse; margin: 0.1in 0 0.16in; font-size: 8.5pt; letter-spacing: -0.005em; }
  thead th { text-align: left; font-size: 7.5pt; font-weight: 700; text-transform: uppercase; letter-spacing: 0.07em; color: var(--body); border-bottom: 1.5px solid var(--civic-navy); padding: 0.05in 0.08in; }
  thead th.r { text-align: right; }
  thead th.c { text-align: center; }
  tbody td { padding: 0.05in 0.08in; border-bottom: 1px solid var(--line); vertical-align: top; }
  tbody tr:nth-child(even) { background: var(--panel); }
  td.n { font-weight: 600; }
  table.exec { font-size: 7.5pt; }
  table.exec thead th { font-size: 6.5pt; }
  table.exec tbody td { padding: 0.035in 0.06in; }
  .gap-tag { display: inline-block; font-size: 6.5pt; font-weight: 700; color: var(--civic-red); border: 1px solid var(--civic-red); border-radius: 2px; padding: 0.01in 0.05in; margin-left: 0.06in; vertical-align: middle; letter-spacing: 0.03em; }
  /* Attr grid */
  .attr-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 0.12in; margin: 0.1in 0 0.16in; }
  .attr-grid table { margin: 0; }
  /* Player cards */
  .player-card { border: 1px solid var(--line); border-top: 3px solid var(--civic-navy); margin-bottom: 0.24in; page-break-inside: avoid; }
  .pc-head { display: flex; justify-content: space-between; align-items: flex-start; padding: 0.16in 0.2in 0.12in; border-bottom: 1px solid var(--line); }
  .pc-id { display: flex; gap: 0.14in; align-items: baseline; }
  .pc-rank { font-family: "SF Pro Display", sans-serif; font-size: 20pt; font-weight: 700; color: var(--civic-red); letter-spacing: -0.02em; }
  .pc-name { font-size: 13pt; font-weight: 700; color: var(--civic-navy); letter-spacing: -0.01em; }
  .pc-sub { font-size: 8pt; color: var(--body); margin-top: 0.03in; }
  .pc-meta { text-align: right; font-size: 8.5pt; font-weight: 600; color: var(--civic-navy); display: flex; flex-direction: column; gap: 0.02in; }
  .pc-meta .dim { font-weight: 400; }
  .pc-stats { display: grid; grid-template-columns: repeat(5, 1fr); gap: 0.08in; padding: 0.1in 0.2in; }
  .pcs { padding: 0.08in 0.1in; background: var(--panel); }
  .pcs-label { font-size: 6.5pt; font-weight: 700; text-transform: uppercase; letter-spacing: 0.08em; color: var(--body); }
  .pcs-big { font-family: "SF Pro Display", sans-serif; font-size: 12pt; font-weight: 700; color: var(--civic-navy); letter-spacing: -0.015em; margin-top: 0.03in; }
  .pcs-sub { font-size: 6.5pt; color: var(--body); margin-top: 0.02in; line-height: 1.35; }
  .pc-charts { display: grid; grid-template-columns: 1fr 1fr; gap: 0.12in; padding: 0.1in 0.2in; }
  figure.chart { border: 1px solid var(--line); padding: 0.08in; }
  figure.chart img { width: 100%; }
  figcaption { font-size: 6.5pt; font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; color: var(--body); margin-bottom: 0.05in; }
  .pc-rationale { padding: 0.1in 0.2in 0.14in; border-top: 1px solid var(--line); }
  .pc-rationale p { font-size: 8.5pt; margin-top: 0.05in; letter-spacing: -0.005em; }
  /* Limitations */
  ul.lims { list-style: none; }
  ul.lims li { padding-left: 0.18in; position: relative; margin-bottom: 0.09in; font-size: 8.5pt; color: var(--body); line-height: 1.45; }
  ul.lims li::before { content: ''; position: absolute; left: 0; top: 0.12in; width: 0.07in; height: 0.07in; background: var(--civic-red); }
  .note { font-size: 7.5pt; color: var(--body); }
  .page-break { page-break-before: always; }
</style>
</head>
<body>

<!-- ============ COVER ============ -->
<div class="page cover">
  <div>
    <div class="cover-org">CF Montréal · Recruitment analytics</div>
    <h1 class="cover-title">CF Montréal scouting report</h1>
    <div class="cover-kicker">Top-20 psych-fit, positive-yield value-spike targets</div>
    <p class="cover-intro">A data-driven shortlist of twenty candidates (age under 23, EUR 400k–2.5M) selected for psychological fit calibrated on successful CF Montréal alumni, strictly positive 12-month forecast yield, value-spike potential (v3 expectile-0.9 tail score) and complementarity with the current squad. Every candidate is simulated through 1,000 Monte Carlo paths under a base and a spike scenario — seed 42, 36 months, annual volatility 0.4688.</p>
  </div>
  <div>
    <div class="cover-meta">
      <div class="meta-box"><div class="lbl">Data vintage</div><div class="val">TM valuations through 2026-06-12 · FM26 attributes (Nov 2025) · SofaScore season stats (Aug 2026)</div></div>
      <div class="meta-box"><div class="lbl">Reproducibility</div><div class="val">Seed 42 everywhere · v2 forecaster SHA-256 1d023e96… · deterministic Rust MC binary</div></div>
      <div class="meta-box"><div class="lbl">Headline results</div><div class="val">20/20 positive yield · 20/20 tail score in basket top 40% · spike-run P(≥1.5×) ≈ 99–100% for 19 of 20</div></div>
      <div class="meta-box"><div class="lbl">Squad fit</div><div class="val">Fills the critical left-winger gap (0 in squad) ×3 and left-back gap (2, both 26+) ×3, then CM/DM/CB/AM depth</div></div>
    </div>
    <div class="cover-footer">
      <div class="stats-strip">
        <div class="stat"><div class="big">1,255</div><div class="label">Players in scout basket</div></div>
        <div class="stat"><div class="big">617</div><div class="label">Coverage-gated pool</div></div>
        <div class="stat"><div class="big">20</div><div class="label">Shortlisted targets</div></div>
        <div class="stat"><div class="big">0.4688</div><div class="label">Annual MC volatility</div></div>
      </div>
    </div>
  </div>
</div>

<!-- ============ EXECUTIVE SUMMARY ============ -->
<div class="page">
  <div class="page-header"><div class="doc">CF Montréal scouting report</div><div class="pg">2</div></div>
  <div class="page-body">
    <div class="section">
      <h2 class="section-title">Executive summary</h2>
      <p class="lede">CF Montréal is a bridge club: it signs young players, develops them and sells them on to stronger leagues — the alumni evidence in section 3. This report ranks the top twenty candidates in the CFM scout basket (age under 23, EUR 400k to 2.5M market value) on psychological fit calibrated on successful alumni, positive 12-month yield (v2 forecast), spike potential (v3 expectile-0.9 tail score) and squad complementarity — the current squad has no left winger and only two left-backs. Every shortlisted player has strictly positive forecast yield and a tail score in the top 40% of the basket.</p>
      <p class="note">Ranked by combined score z(psych_fit) + 0.5·z(tail) + 0.3·z(yield) + 0.4·z(squad_fit). MC numbers are the spike run — drift = v3 expectile-0.9 tail score, seed 42, 1,000 paths, annual vol 0.4688, 36 months.</p>
      <table class="exec">
        <thead><tr><th>#</th><th>Player</th><th>Club</th><th>Pos</th><th class="r">Age</th><th class="r">Value</th><th class="r">Psych</th><th class="r">Tail</th><th class="r">12m yield</th><th class="r">Sofa</th><th class="r">MC med 36m</th><th class="r">P(≥1.5×)</th></tr></thead>
        <tbody>${execRows}</tbody>
      </table>
    </div>
  </div>
  <div class="page-footer"><span>CF Montréal recruitment analytics</span><span>2</span></div>
</div>

<!-- ============ CANDIDATE UNIVERSE ============ -->
<div class="page">
  <div class="page-header"><div class="doc">CF Montréal scouting report — methodology</div><div class="pg">3</div></div>
  <div class="page-body">
    <div class="section">
      <h2 class="section-title">Candidate universe</h2>
      <p class="body-copy">From the pre-merged panel <code>data/cfm_scout_panel.csv</code> (14,769 rows, read-only, never re-merged) the basket filter is: age under 23 as of the 2026-06-12 TM vintage; EUR 400,000 ≤ current market value ≤ EUR 2,500,000; <code>attr_source ≠ staff</code> (real FM database attributes only); club not CF Montréal. The basket holds <strong>1,255 players</strong>. Forecast (f36) coverage is 947/14,769 rows; tail (v3 spike) coverage is 7,962/14,769 — reported honestly, never zero-filled.</p>
      <h3>Psych archetype — calibrated on alumni, not opinion</h3>
      <p class="body-copy">Success label per alumnus: <code>success = current value / transfer fee</code>. Alumni with success ≥ 1.5 and a fee above EUR 300k form the reference group (nine alumni); the top half by success (five alumni) is the archetype calibration set:</p>
      <table><thead><tr><th>Alumnus</th><th class="r">Transfer fee</th><th class="r">Current value</th><th class="r">Success multiple</th></tr></thead><tbody>
        ${ALUMNI.map(r => `<tr><td class="n">${r[0]}</td><td class="r">${r[1]}</td><td class="r">${r[2]}</td><td class="r pos">${r[3]}</td></tr>`).join("")}
      </tbody></table>
      <p class="body-copy">The archetype vector is the mean of non-missing FM attributes over these five alumni (FM26 full attrs, read-only lookup by uid):</p>
      <div class="attr-grid">
        ${[ATTRS.slice(0,4), ATTRS.slice(4,8), ATTRS.slice(8)].map(chunk => `<table><tbody>${chunk.map(([a,v]) => `<tr><td>${a}</td><td class="r pos">${v}</td></tr>`).join("")}</tbody></table>`).join("")}
      </div>
      <p class="body-copy">Hidden personality (pressure, professionalism, ambition, adaptability, temperament, controversy) is present on roughly 32% of panel rows, but the alumni carry no usable hidden values — Koné's ambition 0 and adaptability 1 are FM placeholders outside the 1–20 scale. The hidden-attribute archetype therefore uses the documented bridge-club prior: pressure ≥ 12, professionalism ≥ 12, temperament 12, controversy ≤ 8. Injury proneness and controversy are negative (lower is better).</p>
    </div>
  </div>
  <div class="page-footer"><span>CF Montréal recruitment analytics</span><span>3</span></div>
</div>

<!-- ============ METHODOLOGY ============ -->
<div class="page">
  <div class="page-header"><div class="doc">CF Montréal scouting report — methodology</div><div class="pg">4</div></div>
  <div class="page-body">
    <div class="section">
      <h2 class="section-title">Scoring methodology</h2>
      <h3 class="first">Psych-fit score — exact formula</h3>
      <p class="body-copy">For each candidate and attribute the distance to the archetype is computed in z-units using the basket's per-attribute standard deviation (min 1.0):</p>
      <pre class="formula">z_dist = sqrt( mean over available attrs of ((arch[a] - v) / scale[a])^2 )
         where for NEGATIVE attrs (injury proneness, controversy) the signed
         distance is reversed: ((v - arch[a]) / scale[a])^2

psych_fit = 1 / (1 + exp(K · (z_dist - Z0))),   K = 1.5, Z0 = 1.1</pre>
      <p class="body-copy">The score is computed over the intersection of available candidate attributes and archetype attributes; <code>psy_attrs_used</code> counts the attributes entering the score (max 15 = 11 mental + 4 hidden). The sigmoid maps the distance to [0,1] and is monotone. Sanity check: the calibration alumni themselves score 0.61–0.67 (min 0.614, mean 0.670), all at or above 0.6 as required.</p>

      <h3>Yield, spike and combined rank</h3>
      <p class="body-copy"><strong>Yield</strong> — positive expected appreciation: <code>f36_gap_pct &gt; 0</code> and <code>f36_predicted_12m_mv &gt; current_mv</code>, both strictly required; <code>yield_12m_pct = 100 · f36_gap_pct</code>. <strong>Spike score</strong> — <code>tail_tail_score</code>, the v3 expectile-0.9 12-month log-ratio; all top-20 carry a real tail score, sit in the top 40% of the basket (≥ 0.867) and above the basket spike median (0.845). <strong>Combined rank</strong>, applied over the coverage-gated pool of 617 players:</p>
      <pre class="formula">combined = z(psych_fit) + 0.5·z(tail_score) + 0.3·z(yield_12m_pct) + 0.4·z(squad_fit)</pre>
      <p class="body-copy">Selection constraints: the top-20 must be above the basket psych-fit median (0.573) and spike median (0.845), have positive yield, and respect position caps — maximum three per squad slot — so the shortlist stays complementary rather than concentrated.</p>

      <h3>Squad-fit rules</h3>
      <p class="body-copy"><code>squad_fit = positional need + age fit + value-band fit</code>. Positional need: left winger and left-back score 3 (the squad has zero left wingers and two left-backs); AM/CM/DM score 2; RW/RM/CF/SS/CB/RB/GK score 1 — overstocked at 6 CB, 5 CF, 4 RB, 4 RW, 3 GK. Age fit: 17–21 preferred, the MLS U22 initiative window — +2 if age under 22, else +1. Value band: EUR 400k–1.2M ideal for CFM — +2, else +1.</p>
    </div>
  </div>
  <div class="page-footer"><span>CF Montréal recruitment analytics</span><span>4</span></div>
</div>

<!-- ============ METHODOLOGY CONT ============ -->
<div class="page">
  <div class="page-header"><div class="doc">CF Montréal scouting report — methodology</div><div class="pg">5</div></div>
  <div class="page-body">
    <div class="section">
      <h2 class="section-title">Monte Carlo configuration</h2>
      <p class="body-copy">The deterministic Rust binary <code>footy-mc</code> simulates 36 monthly steps with geometric-Brownian-style value paths: seed 42 everywhere; 1,000 paths per player; annual volatility <code>sigma_ann = 0.4688</code>.</p>
      <p class="body-copy"><strong>Base run</strong> (<code>artifacts/cfm_mc</code>): drift <code>mu_ann = ln(f36_predicted_12m_mv / current_mv)</code> — the annual log drift from the v2 mean forecaster. <strong>Spike run</strong> (<code>artifacts/cfm_mc_spike</code>): drift <code>mu_ann = tail_tail_score</code> — the v3 expectile-0.9 12-month log-ratio is the spike scenario. MC stats are terminal-36-month percentiles; P(≥1.5×), P(≥2×), P(≥3×) and downside P(&lt; 0.5×) are probabilities from the terminal distribution.</p>

      <h3>FBref and SofaScore join rules</h3>
      <p class="body-copy"><strong>SofaScore</strong> (primary): search <code>api.sofascore.com/api/v1/search/all?q=&lt;name&gt;</code>; identity confirmed by normalized name plus Transfermarkt date of birth — 19 of 20 exact DOB matches; Leandersson matches by club plus name, medium confidence. Season line chosen by documented rule: current season, youth national-team competitions excluded, rated lines preferred, then most appearances. Missing values stay blank. <strong>FBref</strong> (secondary): blocked — plain HTTP returns 403 and curl_cffi Chrome impersonation also returns 403 (Cloudflare); the maximum three attempts rule was respected. All twenty players are flagged <code>fbref_unavailable = yes (Cloudflare 403)</code>.</p>
      <p class="note">Coverage counts: SofaScore resolved 20/20 player ids, 20/20 with high or medium confidence, 20/20 season ratings; FBref 0/20 (blocked).</p>
    </div>

    <div class="section" style="margin-top:0.28in">
      <h2 class="section-title">Alumni validation — the evidence base</h2>      <p class="body-copy">All eighteen CF Montréal players who left for a fee in the TM transfers record, with transfer outcome and success multiple (<code>current value / transfer fee</code>). The pattern is the classic bridge-club loop: sign young, develop, sell at a multiple — Crépeau 40× and Binks 36× on tiny fees, Koné 3.1× from EUR 8M to EUR 25M at Watford and Sassuolo.</p>
      <table>
        <thead><tr><th>Player</th><th>Left</th><th>To</th><th class="r">Fee</th><th class="r">Current value</th><th>Current club</th><th class="r">Multiple</th></tr></thead>
        <tbody>${ALUMNI_ALL.map(r => `<tr><td class="n">${r[0]}</td><td>${r[1]}</td><td>${r[2]}</td><td class="r">${r[3]}</td><td class="r">${r[4]}</td><td>${r[5]}</td><td class="r pos">${r[6]}</td></tr>`).join("")}</tbody>
      </table>
    </div>

    <div class="section" style="margin-top:0.28in">
      <h2 class="section-title">Squad gap analysis</h2>
      <p class="body-copy">Current squad of 33 players: mean age 24.2, range 16.9–32.8, total squad value EUR 37.65M. Left winger is the number-one gap; left-back is number two (the age profile is old at the position); AM, CM and DM are medium priority. The shortlist fills gaps in order: LW ×3, LB ×3, CM ×3, DM ×3, CB ×3, AM ×2, GK/CF/RW ×1 each.</p>
      <table><thead><tr><th>Sub-position</th><th class="r">Count</th><th>Assessment</th></tr></thead><tbody>
        ${GAPS.map(g => `<tr><td class="n">${g[0]}</td><td class="r">${g[1]}</td><td>${g[2]}</td></tr>`).join("")}
      </tbody></table>
    </div>
  </div>
  <div class="page-footer"><span>CF Montréal recruitment analytics</span><span>5</span></div>
</div>

<!-- ============ PLAYER DETAILS ============ -->
<div class="page">
  <div class="page-header"><div class="doc">CF Montréal scouting report — top-20 detail</div><div class="pg">6</div></div>
  <div class="page-body">
    <div class="section">
      <h2 class="section-title">Top-20 player detail</h2>
      <p class="lede">Full per-player Monte Carlo fan charts (spike and base runs), psych profile, SofaScore season line and squad-fit rationale. Spike run uses the v3 expectile-0.9 tail score as annual drift; base run uses the v2 forecast. Both runs share seed 42, 1,000 paths, annual vol 0.4688, 36 months. Model scores are not guarantees — see limitations on the final page.</p>
    </div>
  </div>
  <div class="page-footer"><span>CF Montréal recruitment analytics</span><span>5</span></div>
</div>

${P.slice(0, 10).map(p => `
<div class="page">
  <div class="page-header"><div class="doc">CF Montréal scouting report — player detail</div><div class="pg">${p.rank + 6}</div></div>
  <div class="page-body" style="padding-top:0.3in">
    ${p.rank === 1 ? `<div class="section"><h2 class="section-title">Top-20 player detail</h2></div>` : ""}
    <div class="player-card">
      <div class="pc-head">
        <div class="pc-id"><span class="pc-rank">${String(p.rank).padStart(2, "0")}</span>
          <div><div class="pc-name">${esc(p.name)}</div>
          <div class="pc-sub">${esc(p.club)} · ${esc(p.league)} (${esc(p.country)}) · age ${p.age.toFixed(1)} · ${p.pos}${p.squadGap ? ` · <span class="gap-tag">fills ${p.squadGap}</span>` : ""}</div></div>
        </div>
        <div class="pc-meta"><span>${fmtEur(p.mv)}</span><span class="dim">CA ${p.ca} / PA ${p.pa}</span><span class="dim">psych ${p.psychFit.toFixed(3)} · tail ${p.tail.toFixed(2)} · 12m ${pct(p.yield12m / 100)}</span></div>
      </div>
      <div class="pc-stats">
        <div class="pcs"><div class="pcs-label">Psych fit</div><div class="pcs-big">${p.psychFit.toFixed(3)}</div><div class="pcs-sub">${p.topPsych.join(" · ")}</div></div>
        <div class="pcs"><div class="pcs-label">SofaScore</div><div class="pcs-big">${p.sRating}</div><div class="pcs-sub">${p.sGoals}G ${p.sAssists}A · ${p.sApps} apps</div></div>
        <div class="pcs"><div class="pcs-label">MC median 36m</div><div class="pcs-big">${fmtEur(p.mcMedian)}</div><div class="pcs-sub">spike · base ${fmtEur(p.baseMedian)}</div></div>
        <div class="pcs"><div class="pcs-label">10–90% band</div><div class="pcs-big">${fmtEur(p.mcP10)}</div><div class="pcs-sub">to ${fmtEur(p.mcP90)}</div></div>
        <div class="pcs"><div class="pcs-label">P(≥1.5× / ≥2× / ≥3×)</div><div class="pcs-big">${fp(p.p15x)} / ${fp(p.p2x)} / ${fp(p.p3x)}</div><div class="pcs-sub">downside ${pct(p.downside)}</div></div>
      </div>
      <div class="pc-charts">
        <figure class="chart"><figcaption>Spike run — drift = v3 expectile-0.9 tail · seed 42 · 1,000 paths</figcaption><img src="cfm/charts/${p.chart.split("/").pop()}" alt="${esc(p.name)} spike fan chart"></figure>
        <figure class="chart"><figcaption>Base run — drift = v2 forecast · seed 42 · 1,000 paths</figcaption><img src="cfm/charts_base/${p.chartBase.split("/").pop()}" alt="${esc(p.name)} base fan chart"></figure>
      </div>
      <div class="pc-rationale"><div class="pcs-label">Squad fit + rationale</div><p>${esc(p.rationale)} <span class="dim">· squad_fit ${p.squadFit} (gap ${p.squadGap || "none"}) · ${esc(p.sTournament)}, ${esc(p.sSeason)} · confidence ${p.sConfidence} · FBref unavailable (Cloudflare 403)</span></p></div>
    </div>
  </div>
  <div class="page-footer"><span>CF Montréal recruitment analytics</span><span>${p.rank + 6}</span></div>
</div>`).join("")}

${P.slice(10).map(p => `
<div class="page">
  <div class="page-header"><div class="doc">CF Montréal scouting report — player detail</div><div class="pg">${p.rank + 6}</div></div>
  <div class="page-body" style="padding-top:0.3in">
    <div class="player-card">
      <div class="pc-head">
        <div class="pc-id"><span class="pc-rank">${String(p.rank).padStart(2, "0")}</span>
          <div><div class="pc-name">${esc(p.name)}</div>
          <div class="pc-sub">${esc(p.club)} · ${esc(p.league)} (${esc(p.country)}) · age ${p.age.toFixed(1)} · ${p.pos}${p.squadGap ? ` · <span class="gap-tag">fills ${p.squadGap}</span>` : ""}</div></div>
        </div>
        <div class="pc-meta"><span>${fmtEur(p.mv)}</span><span class="dim">CA ${p.ca} / PA ${p.pa}</span><span class="dim">psych ${p.psychFit.toFixed(3)} · tail ${p.tail.toFixed(2)} · 12m ${pct(p.yield12m / 100)}</span></div>
      </div>
      <div class="pc-stats">
        <div class="pcs"><div class="pcs-label">Psych fit</div><div class="pcs-big">${p.psychFit.toFixed(3)}</div><div class="pcs-sub">${p.topPsych.join(" · ")}</div></div>
        <div class="pcs"><div class="pcs-label">SofaScore</div><div class="pcs-big">${p.sRating}</div><div class="pcs-sub">${p.sGoals}G ${p.sAssists}A · ${p.sApps} apps</div></div>
        <div class="pcs"><div class="pcs-label">MC median 36m</div><div class="pcs-big">${fmtEur(p.mcMedian)}</div><div class="pcs-sub">spike · base ${fmtEur(p.baseMedian)}</div></div>
        <div class="pcs"><div class="pcs-label">10–90% band</div><div class="pcs-big">${fmtEur(p.mcP10)}</div><div class="pcs-sub">to ${fmtEur(p.mcP90)}</div></div>
        <div class="pcs"><div class="pcs-label">P(≥1.5× / ≥2× / ≥3×)</div><div class="pcs-big">${fp(p.p15x)} / ${fp(p.p2x)} / ${fp(p.p3x)}</div><div class="pcs-sub">downside ${pct(p.downside)}</div></div>
      </div>
      <div class="pc-charts">
        <figure class="chart"><figcaption>Spike run — drift = v3 expectile-0.9 tail · seed 42 · 1,000 paths</figcaption><img src="cfm/charts/${p.chart.split("/").pop()}" alt="${esc(p.name)} spike fan chart"></figure>
        <figure class="chart"><figcaption>Base run — drift = v2 forecast · seed 42 · 1,000 paths</figcaption><img src="cfm/charts_base/${p.chartBase.split("/").pop()}" alt="${esc(p.name)} base fan chart"></figure>
      </div>
      <div class="pc-rationale"><div class="pcs-label">Squad fit + rationale</div><p>${esc(p.rationale)} <span class="dim">· squad_fit ${p.squadFit} (gap ${p.squadGap || "none"}) · ${esc(p.sTournament)}, ${esc(p.sSeason)} · confidence ${p.sConfidence} · FBref unavailable (Cloudflare 403)</span></p></div>
    </div>
  </div>
  <div class="page-footer"><span>CF Montréal recruitment analytics</span><span>${p.rank + 6}</span></div>
</div>`).join("")}

<!-- ============ LIMITATIONS ============ -->
<div class="page">
  <div class="page-header"><div class="doc">CF Montréal scouting report — limitations</div><div class="pg">27</div></div>
  <div class="page-body">
    <div class="section">
      <h2 class="section-title">Honest limitations</h2>
      <ul class="lims">
        <li><strong>Hidden personality coverage is partial</strong> — roughly 32% of the panel; zero values are FM placeholders and treated as missing. The archetype's hidden component relies on the documented bridge-club prior, not on alumni hidden data.</li>
        <li><strong>The alumni calibration set is small — five players.</strong> It is the best available evidence for a club of this size, but psych-fit differences of a few hundredths should not be over-read.</li>
        <li><strong>FBref coverage is 0/20</strong> — Cloudflare 403; the maximum-attempts rule was respected. SofaScore covers 20/20 with ratings, but season lines vary in competition level — youth leagues, loan clubs, second tiers — stated per player.</li>
        <li><strong>Data vintage mismatch:</strong> FM26 attributes approximate November 2025; TM valuations run through 2026-06-12; SofaScore statistics are live. Loan moves after the FM/TM vintage — Spezia, Mantova, Frosinone, Championship, League Two — are visible in SofaScore but not in the TM transfers table.</li>
        <li><strong>Tail scores are model outputs, not guarantees.</strong> The expectile-0.9 drift is the spike scenario: P(≥1.5×) near 100% under that regime describes what happens if the spike materializes, not a probability-weighted forecast. Base-run medians are the conservative comparison.</li>
        <li><strong>Single-window model scores are not guarantees</strong> of future transfer outcomes; they rank candidates within this basket and vintage only.</li>
      </ul>
      <p class="note" style="margin-top:0.2in">Generated by the CFM scouting pipeline (STEERING_CFM.md) · seed 42 · models unchanged (hashes on page 5) · all figures in EUR · as-of vintage: TM 2026-06-12, FM26 attrs Nov 2025, SofaScore Aug 2026. Typeface: SF Pro Text body, New York titles.</p>
    </div>
  </div>
  <div class="page-footer"><span>CF Montréal recruitment analytics</span><span>27</span></div>
</div>

</body>
</html>`;

const outDir = "/home/drewclaw/footy-models/artifacts/cfm_report";
fs.mkdirSync(outDir, { recursive: true });
const outPath = path.join(outDir, "CFM_Scout_Report_design.html");
fs.writeFileSync(outPath, html);
console.log("Wrote", outPath, (html.length / 1024).toFixed(0) + "KB");
console.log("Players:", P.length, "| first:", P[0].name, "| last:", P[P.length - 1].name);
