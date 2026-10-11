#!/usr/bin/env python3
"""Produce auditable OOS event-model evaluation report from saved StatsBomb scores."""
import argparse,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def fmt(x):return f'{x:.4f}'
def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--source',default='artifacts/validation/premier-league-events-2015-16-oos.json')
    p.add_argument('--output',default='docs/premier-league-player-event-oos-validation.md')
    a=p.parse_args();j=json.loads((ROOT/a.source).read_text());m=j['metrics'];ci=j['paired_fixture_bootstrap'];split=j['split']
    names={'player_shot_occurrence_brier':('Player shot occurrence Brier','reference_player_shot_brier'),
      'player_goal_occurrence_brier':('Player goal occurrence Brier','reference_player_goal_brier'),
      'player_assist_occurrence_brier':('Player assist occurrence Brier','reference_player_assist_brier'),
      'minute_goal_brier':('At-least-one goal per 1-minute bin Brier','reference_minute_goal_brier'),
      'minute_assist_brier':('At-least-one assist per 1-minute bin Brier','reference_minute_assist_brier'),
      'window15_goal_brier':('At-least-one goal per 15-minute bin Brier','reference_window15_goal_brier'),
      'team_shot_count_mae':('Team shot count MAE','reference_team_shot_mae')}
    rows=['# Out-of-sample Premier League player-event calibration audit','',
      '## Audit conclusion','',
      '**Current 2026 FM26 minute/player simulator has NOT been statistically validated here.** This run evaluates a simple date-causal player/team event-rate baseline against actual goals, registered assists, shots, and goal/assist timing in a historical Premier League season. The held-out evaluation is genuine, but a different model and time period. Avoid transferring its metrics to the Rust FM26 match simulator.','',
      f"**Data:** StatsBomb open-data cached English PL 2015/16 events; {j['dataset_matches_with_events']} event-covered matches of 380 season fixtures. Other {j['metadata_matches_without_event_files']} metadata fixtures lacked event files and were not evaluated.",
      f"**Holdout:** {split['holdout_games']} chronologically last covered fixtures, from {split['earliest_test_date']} to {split['latest_test_date']}. For each prediction use only event-covered fixtures with `match_date < predicted_match_date`. No same-day events, future roster, FM26, or test-match starting XI may enter feature calculation.",
      '', '## Real out-of-sample performance','',
      '| Held-out metric (lower = better) | Causal historical-rate model | Simple previous-only reference | Model minus reference |',
      '|---|---:|---:|---:|']
    for key,(label,reference) in names.items():
        rows.append(f'| {label} | {fmt(m[key])} | {fmt(m[reference])} | {m[key]-m[reference]:+.5f} |')
    rows+=['','### Match-paired sampling uncertainty','',
        '| Metric | Model − baseline (negative favors model) | 95% fixture bootstrap interval | Improvement established at 95%? |',
        '|---|---:|---|---|']
    for k,v in ci.items():
        rows.append(f"| {k.replace('_',' ')} | {v['model_minus_reference_mean']:+.6f} | [{v['paired_fixture_bootstrap_95'][0]:+.6f}, {v['paired_fixture_bootstrap_95'][1]:+.6f}] | {'yes' if v['clear_improvement_at_95'] else 'no'} |")
    rows+=['',
      '**Interpretation:** Shots modestly improve as a point estimate, but the match bootstrap overlaps zero. Goal timing is marginally worse than uniform historical timing; player goal Brier is worse than the simple reference. More Monte Carlo paths would not fix those features. The minute-level assist baseline also performs worse. No prospective reliability claim is warranted. These intervals do not fully correct team/fixture dependence in an incomplete open-data subset.','',
      '## Coverage / actual events','',
      f"Across the {split['holdout_games']} held-out matches, {m['unseen_player_actual_goals']:.0f} actual goals and {m['unseen_player_actual_assists']:.0f} registered assists came from players absent from the causal pre-match player universe. Known-player coverage averages {m['prior_known_players_per_match']:.1f} candidates across both clubs, **not** confirmed lineup participants.",
      '', '| Date | Actual fixture | Real final score | Pre-match event-rate expected goals | Actual recorded goal minutes (0-indexed regulation bins) |',
      '|---|---|---|---|---|']
    for g in j['games'][-10:]:
        home,away=g['home'],g['away'];actual=g['actual_score'];expect=g['predicted_goals']
        ev=', '.join(f"{r['team'].split()[0]} {r['minute_index']+1}′" for r in sorted(g['goal_event_times'],key=lambda x:x['minute_index'])) or 'none'
        rows.append(f"| {g['date']} | {home} – {away} | {actual[0]}–{actual[1]} | {expect[0]:.2f}–{expect[1]:.2f} | {ev} |")
    rows+=['','## Event attribution and limitations','',
      'Actual shot records are StatsBomb `Shot` objects; actual goals are `Shot` with outcome `Goal`, plus own-goal accounting where relevant. Actual assists are `Pass.goal_assist=true` (this is **not** `xA` and not necessarily every informal secondary assist). Timestamps in the data include clock minute and second; after 90′, added-time observations are assigned to minute 90 for 90-bin conservation, never treated as an exactly timed 90th-minute event.',
      '', 'The benchmark predicts player probabilities of at least one goal/assist/shot from strictly earlier observed appearances, team rates, and historical event timing. It **does not** predict goalscorer / assist identities from FM attributes, expected lineups, or dynamic psychology. The **current Rust simulator** now outputs 90 one-minute marginal event bins and player-scorer/provisional-assist allocations, but its scorer weights and fixed assist probability have not yet been backtested on historical as-of FM/team inputs.',
      '', 'Required for a credible prospective test: dated squads/injuries, contemporaneous FM snapshots or non-leaky historic player signals, confirmed postgame scorers/assists/shots, a pre-frozen calibration protocol, and reporting of 1-minute and player-event Brier/MAE/log loss against both uniform and historical-rate baselines with matchday-block uncertainty. Hold the chronological test portion out of tuning.',
      '', '## Reproducible scripts','',
      '```bash',
      'python3 scripts/backtest_pl_event_oos.py --holdout-matches 24',
      'python3 scripts/write_pl_oos_report.py',
      'python3 -m unittest discover -s tests -p test_pl_event_oos.py',
      '```','',
      'All raw StatsBomb event files remain local; scored aggregate, source metadata, actual observed event timestamps, and per-match predictions are saved in [`artifacts/validation/premier-league-events-2015-16-oos.json`](../artifacts/validation/premier-league-events-2015-16-oos.json).','']
    target=ROOT/a.output;target.parent.mkdir(parents=True,exist_ok=True);target.write_text('\n'.join(rows),encoding='utf8');print('Wrote',target)
if __name__=='__main__':main()
