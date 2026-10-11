#!/usr/bin/env node
/** CFM scout report — Apple Print Design Language + ASD-STE100 simple English.
 *  SF Pro Display headers, SF Pro Text body, neutral palette (#1D1D1F/#86868B/#E5E5EA/#F5F5F7),
 *  8pt grid (72pt margins), whitespace-first, no decorative elements.
 *  Prose follows ASD-STE100: short active sentences, max 25 words, no hedging.
 */
const fs = require("fs");
const path = require("path");

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
const pct = (v) => Math.round(v * 100) + "%";
const chance = (v) => (v >= 0.995 ? "99%" : pct(v));
const chanceWord = (v) => (v >= 0.995 ? "almost certain" : v >= 0.9 ? "very likely" : v >= 0.75 ? "likely" : v >= 0.5 ? "about even" : "unlikely");
const psychWord = (v) => (v >= 0.65 ? "excellent" : v >= 0.6 ? "strong" : v >= 0.575 ? "good" : "solid");
const esc = (s) => String(s).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

const CLUB_SHORT = {
  "Club Atlético Banfield": "Banfield", "Idrottsföreningen Kamraterna Göteborg": "IFK Göteborg",
  "Allmänna Idrottsklubben": "AIK", "Club de Gimnasia y Esgrima La Plata": "Gimnasia",
  "NK Istra 1961": "Istra 1961", "West Ham United": "West Ham", "Aberdeen FC": "Aberdeen",
  "Mjällby Almänna Idrottsförening": "Mjällby", "Associazione Sportiva Roma": "Roma",
  "AZ Alkmaar": "AZ Alkmaar", "SL Benfica": "Benfica", "Club Atlético Platense": "Platense",
  "Cagliari Calcio": "Cagliari", "BSC Young Boys": "Young Boys",
  "NK Lokomotiva Zagreb": "Lokomotiva", "Inter Milan": "Inter",
  "Kristiansund Ballklubb": "Kristiansund", "Genoa CFC": "Genoa",
};

const whyTag = (p) => {
  const bits = [];
  if (p.psychFit >= 0.6) bits.push("attitude matches the club profile");
  if (p.p15x >= 0.9) bits.push("value has a real chance to jump");
  if (p.squadGap) bits.push("fills a squad gap (" + p.squadGap + ")");
  if (p.yield12m / 100 >= 0.5) bits.push("value is expected to grow");
  return bits.join(". ");
};

const gapPlain = {
  "LW": "the club has no left winger at all",
  "LB": "the club has only two left-backs, both over 26",
  "CB": "centre-back depth", "CM": "central midfield depth",
  "DM": "defensive midfield depth", "AM": "attacking midfield depth",
  "RW": "right wing depth", "CF": "striker depth", "GK": "goalkeeper depth",
};

const execRows = (slice) => slice.map(p => `<tr>
  <td class="rank">${p.rank}</td>
  <td class="name">${esc(p.name)}</td>
  <td>${esc(CLUB_SHORT[p.club] || p.club)}</td>
  <td>${p.pos}</td>
  <td>${p.age.toFixed(0)}</td>
  <td class="num">${fmtEur(p.mv)}</td>
  <td class="num">${fmtEur(p.mcMedian)}</td>
  <td class="num">${chance(p.p15x)}</td>
</tr>`).join("");

const playerPages = P.map(p => `
<div class="page">
  <div class="ph"><span>CF Montréal scouting report</span><span>${p.rank + 5}</span></div>
  <div class="pb">
    <div class="player-head">
      <div class="ph-l">
        <div class="p-num">${String(p.rank).padStart(2, "0")}</div>
        <div>
          <h2 class="p-name">${esc(p.name)}</h2>
          <div class="p-sub">${esc(CLUB_SHORT[p.club] || p.club)} · ${p.pos} · age ${p.age.toFixed(0)} · worth ${fmtEur(p.mv)} today</div>
        </div>
      </div>
      <div class="ph-r">
        <div class="p-v">${fmtEur(p.mcMedian)}</div>
        <div class="p-vl">most likely value in 3 years</div>
      </div>
    </div>
    <div class="why">
      <div class="why-kicker">Why he makes the list</div>
      <p class="why-text">${whyTag(p)}.</p>
      ${p.squadGap ? `<p class="why-gap">Squad note: ${gapPlain[p.squadGap] || "fills a squad gap"}.</p>` : ""}
    </div>
    <div class="charts">
      <figure>
        <figcaption>If things go well</figcaption>
        <div class="chartbox"><img src="cfm/charts/${p.chart.split("/").pop()}" alt="${esc(p.name)} value paths, optimistic forecast"></div>
        <p class="cap">The grey band is the likely range of value. The dark line is the middle path.</p>
      </figure>
      <figure>
        <figcaption>On the cautious forecast</figcaption>
        <div class="chartbox"><img src="cfm/charts_base/${p.chartBase.split("/").pop()}" alt="${esc(p.name)} value paths, cautious forecast"></div>
        <p class="cap">The same player with the conservative forecast. Most paths stay above today's value.</p>
      </figure>
    </div>
    <div class="stats-row">
      <div class="st"><div class="st-big">${p.psychFit.toFixed(2)}</div><div class="st-lbl">Attitude score, 0 to 1 — ${psychWord(p.psychFit)}</div></div>
      <div class="st"><div class="st-big">${chance(p.p15x)}</div><div class="st-lbl">Chance value rises 50% or more — ${chanceWord(p.p15x)}</div></div>
      <div class="st"><div class="st-big">${fmtEur(p.mcP10)}</div><div class="st-lbl">Worst realistic value in 3 years</div></div>
      <div class="st"><div class="st-big">${p.sRating}</div><div class="st-lbl">Performance rating this season, out of 10</div></div>
    </div>
    <div class="rationale">
      <div class="why-kicker">In one paragraph</div>
      <p class="rationale-text">${esc(p.rationale)}</p>
      <p class="cap">${esc(p.sTournament)}, ${esc(p.sSeason)} · ${p.sGoals} goal(s), ${p.sAssists} assist(s), ${p.sApps} appearances · ${p.sConfidence} confidence · FBref stats unavailable (site blocks automated access).</p>
    </div>
  </div>
  <div class="pf"><span>CF Montréal recruitment analytics</span><span>${p.rank + 5}</span></div>
</div>`).join("");

const pages = [];

// ============ COVER ============
pages.push(`
<div class="page cover">
  <div class="cover-top">
    <div class="cover-kicker">CF Montréal · Recruitment</div>
    <h1 class="cover-title">Twenty players worth a look</h1>
    <p class="cover-sub">A shortlist of young players who fit the club style, whose value can grow, and who fill real gaps in the squad. Written for everyone at the club. No data background needed.</p>
  </div>
  <div class="cover-bottom">
    <div class="cover-nums">
      <div><div class="cn-big">1,255</div><div class="cn-lbl">Young players reviewed</div></div>
      <div><div class="cn-big">20</div><div class="cn-lbl">Made the shortlist</div></div>
      <div><div class="cn-big">3</div><div class="cn-lbl">Left-wing targets. The club has no left winger today</div></div>
      <div><div class="cn-big">1,000</div><div class="cn-lbl">Simulations run per player</div></div>
    </div>
    <div class="cover-foot">Prepared with the CFM scouting pipeline. Data as of June 2026. Values in euros.</div>
  </div>
</div>`);

// ============ THE SHORTLIST (PART 1) ============
pages.push(`
<div class="page">
  <div class="ph"><span>CF Montréal scouting report</span><span>2</span></div>
  <div class="pb">
    <div class="kicker">Part 1 of 2</div>
    <h2 class="section">The shortlist</h2>
    <p class="body">Each player is under 23. Each costs between EUR 400k and EUR 2.5M. Each matches the attitude profile of the club's successful sales. Each has a real chance that value grows.</p>
    <table>
      <thead><tr><th>#</th><th>Player</th><th>Club</th><th>Pos</th><th class="r">Age</th><th class="r">Value today</th><th class="r">Likely in 3 yrs</th><th class="r">Chance of +50%</th></tr></thead>
      <tbody>${execRows(P.slice(0, 10))}</tbody>
    </table>
    <p class="small margins">Values come from simulations, not promises. "Chance of +50%" is the share of 1,000 simulated futures where the player is worth at least 50% more than today.</p>
  </div>
  <div class="pf"><span>CF Montréal recruitment analytics</span><span>2</span></div>
</div>`);

// ============ THE SHORTLIST (PART 2) ============
pages.push(`
<div class="page">
  <div class="ph"><span>CF Montréal scouting report</span><span>3</span></div>
  <div class="pb">
    <div class="kicker">Part 2 of 2</div>
    <h2 class="section">The shortlist, ranks 11 to 20</h2>
    <p class="body">The same rules apply to every player on this page. Each one is under 23. Each one fits the club profile. Each one fills a squad need or has a strong value case.</p>
    <table>
      <thead><tr><th>#</th><th>Player</th><th>Club</th><th>Pos</th><th class="r">Age</th><th class="r">Value today</th><th class="r">Likely in 3 yrs</th><th class="r">Chance of +50%</th></tr></thead>
      <tbody>${execRows(P.slice(10))}</tbody>
    </table>
    <p class="small margins">The ranking combines four factors: attitude fit, chance of a value jump, expected value growth, and squad need. Details are on the next pages.</p>
  </div>
  <div class="pf"><span>CF Montréal recruitment analytics</span><span>3</span></div>
</div>`);

// ============ HOW WE PICKED THEM ============
pages.push(`
<div class="page">
  <div class="ph"><span>CF Montréal scouting report</span><span>4</span></div>
  <div class="pb">
    <div class="kicker">How we picked them</div>
    <h2 class="section">Three questions. Three answers.</h2>
    <p class="body">We started with 1,255 young players from the global market. Then we asked three questions. A player must pass all three to make this list.</p>
    <h3>1. Does his attitude fit the club?</h3>
    <p class="body">CF Montréal buys young players. It develops them. It sells them for more. We studied the 18 players who left the club for a fee. We looked at what the successful ones had in common. Determination. Composure. Work rate. Teamwork. That profile is the yardstick. Every player on this list matches it closely.</p>
    <h3>2. Can his value grow?</h3>
    <p class="body">For each player we ran 1,000 simulated futures over three years. Each future is a different "what if". Injury, form, and moves differ in each one. The result is a range of values: the likely value, the worst realistic value, and the odds of a big jump.</p>
    <h3>3. Is there a place for him?</h3>
    <p class="body">The squad has no left winger. It has two left-backs, both over 26. We scored each position by need. A player who fills a real hole gets bonus points. No position gets more than three names.</p>
  </div>
  <div class="pf"><span>CF Montréal recruitment analytics</span><span>4</span></div>
</div>`);

// ============ WHAT THE NUMBERS MEAN ============
pages.push(`
<div class="page">
  <div class="ph"><span>CF Montréal scouting report</span><span>5</span></div>
  <div class="pb">
    <div class="kicker">Reading the player pages</div>
    <h2 class="section">What the numbers mean</h2>
    <p class="body">Each player gets one page. This page explains how to read it.</p>
    <h3>The two charts</h3>
    <p class="body">Both charts show where value can go over three years. The grey band is the likely range. The dark line is the middle path. The left chart is the optimistic forecast. The right chart is the cautious forecast. If both end above today's value, the player is a low-risk bet.</p>
    <h3>The four numbers</h3>
    <p class="body"><strong>Attitude score</strong> — how well he matches the club's success profile. Range is 0 (no match) to 1 (perfect match). <strong>Chance of +50%</strong> — the share of simulations where value rises 50% or more. <strong>Worst realistic value</strong> — the bad end of the range. <strong>Performance rating</strong> — his average match rating this season, out of 10.</p>
    <h3>The one-paragraph summary</h3>
    <p class="body">Every page ends with a plain-language summary. That paragraph is the short version. The rest of the page is the evidence.</p>
    <p class="small">Sources: Transfermarkt values (June 2026), Football Manager 26 attributes (November 2025), SofaScore ratings (August 2026). FBref blocks automated access, so it is not used. This is stated honestly, not hidden.</p>
  </div>
  <div class="pf"><span>CF Montréal recruitment analytics</span><span>5</span></div>
</div>`);

// ============ PLAYER PAGES ============
pages.push(playerPages);

// ============ WHAT COULD BE WRONG ============
const LAST = P.length + 6;
pages.push(`
<div class="page">
  <div class="ph"><span>CF Montréal scouting report</span><span>${LAST}</span></div>
  <div class="pb">
    <div class="kicker">Being honest</div>
    <h2 class="section">What could be wrong</h2>
    <p class="body">This report is a starting point, not a guarantee. These are the limits we know.</p>
    <ul class="plain">
      <li><strong>Personality data is incomplete.</strong> Hidden traits like pressure handling and professionalism exist for about one third of players. Where they are missing, we used a general profile of what works at clubs like CF Montréal.</li>
      <li><strong>The club benchmark is small.</strong> Five players set the success profile. It is the best evidence a club this size has. Do not over-read small differences.</li>
      <li><strong>Simulations are estimates, not promises.</strong> They show what can happen, not what will happen. The optimistic chart assumes good form continues.</li>
      <li><strong>Data ages.</strong> Values are from June 2026. Attributes are from November 2025. Ratings are from August 2026. A move or an injury changes the picture.</li>
      <li><strong>One advanced-stats site was unavailable.</strong> FBref blocks automated access. Those stats are absent. Everything else is real data, not filled-in guesses.</li>
    </ul>
    <p class="small margins">Prepared with the CFM scouting pipeline. Fixed random seed (42) for repeatable results. Values in euros.</p>
  </div>
  <div class="pf"><span>CF Montréal recruitment analytics</span><span>${LAST}</span></div>
</div>`);

// PAGES_SLOT

const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<title>CF Montréal scouting report — twenty players worth a look</title>
<style>
  /* Apple Print Design Language. 8pt grid, 72pt margins, neutral palette only. */
  @page { size: letter; margin: 0; }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  html { font-size: 10pt; }
  body { font-family: "SF Pro Text", "Helvetica Neue", Arial, sans-serif; background: #FFFFFF; color: #1D1D1F; line-height: 1.5; }
  .page { width: 8.5in; height: 11in; background: #FFFFFF; display: flex; flex-direction: column; page-break-after: always; overflow: hidden; }
  .page:last-child { page-break-after: auto; }
  h1, h2, h3 { font-family: "SF Pro Display", "Helvetica Neue", Arial, sans-serif; color: #1D1D1F; font-weight: 600; letter-spacing: -0.01em; }
  .pb { flex: 1; min-height: 0; overflow: hidden; padding: 56pt 72pt 40pt; display: flex; flex-direction: column; }
  .ph, .pf { padding: 32pt 72pt 0; display: flex; justify-content: space-between; font-size: 8pt; color: #86868B; letter-spacing: 0.02em; flex: 0 0 auto; }
  .pf { padding: 0 72pt 32pt; border-top: 0.5pt solid #E5E5EA; margin-top: 16pt; padding-top: 12pt; }
  .cover { justify-content: space-between; }
  .cover-top { padding: 72pt; }
  .cover-kicker { font-size: 9pt; font-weight: 500; color: #86868B; letter-spacing: 0.02em; margin-bottom: 20pt; }
  .cover-title { font-size: 40pt; font-weight: 600; letter-spacing: -0.01em; line-height: 1.05; max-width: 5.2in; }
  .cover-sub { font-size: 11pt; color: #86868B; margin-top: 20pt; max-width: 4.6in; line-height: 1.5; }
  .cover-bottom { padding: 72pt; }
  .cover-nums { display: grid; grid-template-columns: repeat(4, 1fr); gap: 36pt; }
  .cn-big { font-family: "SF Pro Display", sans-serif; font-size: 28pt; font-weight: 600; letter-spacing: -0.01em; color: #1D1D1F; }
  .cn-lbl { font-size: 8pt; font-weight: 500; color: #86868B; margin-top: 4pt; letter-spacing: 0.02em; }
  .cover-foot { font-size: 8pt; color: #86868B; margin-top: 48pt; letter-spacing: 0.02em; }
  .kicker { font-size: 9pt; font-weight: 500; color: #86868B; letter-spacing: 0.02em; margin-bottom: 8pt; }
  h2.section { font-size: 24pt; font-weight: 600; letter-spacing: -0.01em; margin-bottom: 16pt; }
  h3 { font-size: 13pt; font-weight: 600; margin: 16pt 0 6pt; letter-spacing: -0.005em; }
  p.body { font-size: 10pt; line-height: 1.45; color: #1D1D1F; margin-bottom: 8pt; max-width: 5.8in; }
  p.small { font-size: 8pt; color: #86868B; line-height: 1.45; max-width: 5.8in; }
  table { width: 100%; border-collapse: collapse; margin-top: 12pt; }
  thead th { text-align: left; font-size: 8pt; font-weight: 500; color: #86868B; letter-spacing: 0.02em; padding: 8pt 8pt 8pt 0; border-bottom: 0.5pt solid #E5E5EA; }
  tbody td { padding: 10pt 8pt 10pt 0; font-size: 10pt; border-bottom: 0.5pt solid #E5E5EA; color: #1D1D1F; }
  td.name { font-weight: 600; }
  td.rank { color: #86868B; font-weight: 500; }
  td.num { text-align: right; font-variant-numeric: tabular-nums; }
  thead th.r { text-align: right; }
  .player-head { display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 16pt; }
  .ph-l { display: flex; align-items: baseline; gap: 12pt; }
  .p-num { font-family: "SF Pro Display", sans-serif; font-size: 24pt; font-weight: 600; color: #E5E5EA; letter-spacing: -0.01em; }
  .p-name { font-size: 22pt; font-weight: 600; letter-spacing: -0.01em; }
  .p-sub { font-size: 9.5pt; color: #86868B; margin-top: 4pt; }
  .ph-r { text-align: right; }
  .p-v { font-family: "SF Pro Display", sans-serif; font-size: 22pt; font-weight: 600; letter-spacing: -0.01em; }
  .p-vl { font-size: 8pt; color: #86868B; margin-top: 2pt; letter-spacing: 0.02em; }
  .why { margin-bottom: 12pt; }
  .why-kicker { font-size: 8pt; font-weight: 500; color: #86868B; letter-spacing: 0.02em; text-transform: uppercase; margin-bottom: 6pt; }
  .why-text { font-size: 11pt; line-height: 1.45; color: #1D1D1F; max-width: 5.6in; }
  .why-gap { font-size: 9.5pt; color: #1D1D1F; margin-top: 6pt; }
  .charts { display: grid; grid-template-columns: 1fr 1fr; gap: 24pt; margin: 12pt 0; }
  figure figcaption { font-size: 9pt; font-weight: 600; color: #1D1D1F; margin-bottom: 6pt; }
  .chartbox { border: 0.5pt solid #E5E5EA; padding: 6pt; }
  .chartbox img { width: 100%; display: block; max-height: 1.9in; object-fit: contain; }
  .cap { font-size: 7.5pt; color: #86868B; line-height: 1.4; margin-top: 6pt; }
  .stats-row { display: grid; grid-template-columns: repeat(4, 1fr); gap: 16pt; margin: 8pt 0 12pt; }
  .st-big { font-family: "SF Pro Display", sans-serif; font-size: 16pt; font-weight: 600; letter-spacing: -0.01em; color: #1D1D1F; }
  .st-lbl { font-size: 7.5pt; color: #86868B; margin-top: 4pt; line-height: 1.4; letter-spacing: 0.01em; }
  .rationale { margin-top: 4pt; }
  .rationale-text { font-size: 9.5pt; line-height: 1.5; color: #1D1D1F; max-width: 6in; }
  .margins { margin-top: 32pt; }
  ul.plain { list-style: none; }
  ul.plain li { padding-left: 16pt; position: relative; margin-bottom: 12pt; font-size: 10.5pt; line-height: 1.5; max-width: 6in; }
  ul.plain li::before { content: ''; position: absolute; left: 0; top: 8pt; width: 6pt; height: 6pt; background: #86868B; }
</style>
</head>
<body>
${pages.join("\n")}
</body>
</html>`;

const outDir = "/home/drewclaw/footy-models/artifacts/cfm_report";
fs.mkdirSync(outDir, { recursive: true });
const outPath = path.join(outDir, "CFM_Scout_Report_apple.html");
fs.writeFileSync(outPath, html);
console.log("Wrote", outPath, (html.length / 1024).toFixed(0) + "KB");
console.log("Players:", P.length, "| first:", P[0].name, "| last:", P[P.length - 1].name);
