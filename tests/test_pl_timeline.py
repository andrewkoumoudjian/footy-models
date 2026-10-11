"""Regression tests for honest 15-minute plotting and single-command pipeline."""
import json
import math
import tempfile
import xml.etree.ElementTree as ET
import subprocess
import sys
import unittest
from pathlib import Path
from PIL import Image

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
from render_pl_timeline import momentum,derived,timeline_figure,overview_figure,W,H

class TimelineTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.snapshot=json.loads((ROOT/'artifacts/premier-league-2026-10-11/player_paths.json').read_text())
        cls.derived=json.loads((ROOT/'assets/predictions/2026-10-11/momentum_15min.json').read_text())

    def test_momentum_is_symmetric_and_clamped(self):
        template={'expected_home_shots':3,'expected_away_shots':1,
                  'expected_home_goals':.4,'expected_away_goals':.1}
        actual=momentum(template)
        self.assertAlmostEqual(actual,100*(3+3*.4-(1+3*.1))/(3+3*.4+1+3*.1))
        swapped={**template,'expected_home_shots':template['expected_away_shots'],
                 'expected_away_shots':template['expected_home_shots'],
                 'expected_home_goals':template['expected_away_goals'],
                 'expected_away_goals':template['expected_home_goals']}
        self.assertAlmostEqual(momentum(swapped),-actual)
        self.assertEqual(momentum({k:0 for k in template}),0)
        with self.assertRaises(ValueError):momentum({**template,'expected_home_goals':-1})

    def test_derived_matches_simulator_and_is_bounded(self):
        fixtures=self.snapshot['fixtures']
        self.assertEqual(len(fixtures),4)
        for fixture in fixtures:
            raw=fixture['15_minute_windows']
            points=derived(fixture)
            recorded=self.derived['fixtures'][f"{fixture['home']} vs {fixture['away']}"]
            self.assertEqual(len(points),6)
            for idx,(point,original,committed) in enumerate(zip(points,raw,recorded)):
                self.assertEqual((point['window_start'],point['window_end']),(15*idx,15*(idx+1)))
                self.assertGreaterEqual(point['momentum_proxy'],-100)
                self.assertLessEqual(point['momentum_proxy'],100)
                self.assertAlmostEqual(point['momentum_proxy'],committed['momentum_proxy'])
                self.assertAlmostEqual(point['probability_red_card'],original['p_red_card'])
                self.assertAlmostEqual(sum([point['home_lead'],point['draw_state'],point['away_lead']]),1,places=5)
            self.assertAlmostEqual(sum(p['expected_home_goals'] for p in points),fixture['expected_home_goals'])
            self.assertAlmostEqual(sum(p['expected_away_goals'] for p in points),fixture['expected_away_goals'])

    def test_no_fabricated_event_classes_or_interpolation(self):
        self.assertEqual(self.derived['events_included'],
            ['goals','shots','red cards','substitution triggers','score-state'])
        self.assertEqual(self.derived['not_modelled'],
            ['yellow cards','corners','possession','fouls','exact per-event timestamps'])
        for f in self.snapshot['fixtures']:
            fig=timeline_figure(f,self.snapshot['as_of'])
            obj=fig.to_plotly_json()
            self.assertEqual(obj['layout']['width'],W)
            self.assertEqual(obj['layout']['height'],H)
            self.assertEqual(obj['layout']['xaxis']['range'],[-3,93])
            self.assertEqual(len(obj['data']),11)
            self.assertEqual(fig.to_json(),timeline_figure(f,self.snapshot['as_of']).to_json())
            for label,category in [(f"{f['home']} lead",'home_lead'),('level','draw_state'),(f"{f['away']} lead",'away_lead')]:
                row=next(x for x in obj['data'] if x.get('name')==label)
                self.assertEqual(list(row['x']),[0,15,30,45,60,75,90])
                self.assertAlmostEqual(list(row['y'])[0],100.0 if category=='draw_state' else 0.0)
        overview=overview_figure(self.snapshot['fixtures'],self.snapshot['as_of'])
        self.assertGreaterEqual(overview.layout.height,850)

    def test_readme_png_and_svg_appear(self):
        readme=(ROOT/'README.md').read_text()
        for i,slug in enumerate(['crystal-palace','hull','liverpool','coventry'],1):
            stem=f'{i:02d}-{slug}'
            for ext in ['.png','.svg']:
                path=ROOT/'assets/predictions/2026-10-11'/f'{stem}{ext}'
                self.assertTrue(path.exists(),path)
                self.assertIn(str(path.relative_to(ROOT)),readme)
            self.assertTrue(path.with_suffix('.html').exists())
            svg=ET.parse(path.with_suffix('.svg')).getroot()
            self.assertTrue(svg.tag.endswith('svg'))
            with Image.open(path.with_suffix('.png')) as im:
                self.assertEqual(im.size,(W,H))
        self.assertIn('assets/predictions/2026-10-11/overview.png',readme)

    def test_pipeline_dry_run_has_all_stages(self):
        with tempfile.NamedTemporaryFile() as fm:
            command=[sys.executable,'scripts/run_pl_pipeline.py','--dry-run','--offline',
                     '--fm-file',fm.name]
            out=subprocess.run(command,cwd=ROOT,text=True,capture_output=True,check=True)
        for stage in ['forecast_premier_league.py','match_premier_league_fm26.py',
                      'prepare_pl_player_inputs.py','footy-matchforecast-pl',
                      'validate_pl_snapshot.py','render_pl_timeline.py','write_pl_timeline_report.py']:
            self.assertIn(stage,out.stdout)

if __name__=='__main__':unittest.main()
