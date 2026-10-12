"""Regression tests for one pooled, interpretable structured football model."""
from datetime import timedelta, date
import math
import unittest

from shared_math_glm import (FEATURE_NAMES, fit, proper_loss, score, validate_splits)


def fixtures():
    rows=[]
    for split, year, size in (('train',2023,90),('validation',2024,35),('test',2025,35)):
        for i in range(size):
            values=dict.fromkeys(FEATURE_NAMES,None)
            values['fm_attributes_observed']=float(i%3!=0)
            values['fm_mental_Composure']=float(8+(i%7)) if i%3 else None
            values['fm_mental_Leadership']=float(9+(i%6)) if i%3 else None
            values['sofascore_rating']=6.0+(i%8)/10 if i%4 else None
            values['fbref_shots_per90']=0.3+(i%10)/4 if i%5 else None
            values['team_mental_coverage']=0.55+(i%4)/10
            values['position_mf']=1.0
            values['league_mls']=float(i%2)
            y_start=int(i%7!=0)
            minutes= 90.0 if y_start else (25.0 if i%3==0 else 0.0)
            shots=(i%4) if minutes>0 else 0
            goals=1 if shots>0 and i%11==0 else 0
            d=date(year,1,1)+timedelta(days=i//4)
            rows.append({
                'split':split,'date':d.isoformat(),
                'feature_cutoff_utc':(d-timedelta(days=1)).isoformat()+'T18:00:00Z',
                'league':'EPL' if i%2==0 else 'MLS',
                'fixture_id':f'{year}-{i//4}-{i%2}',
                'canonical_player_id':f'fm26:{i}',
                'features':values,
                'label':{'started':bool(y_start),'minutes':minutes,
                         'shots':shots,'goals':goals},
            })
    return rows


class SharedMathGLMTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rows=fixtures()
        cls.model=fit(cls.rows,penalty=1.0)

    def test_one_model_both_leagues_neither_identity_coefficients(self):
        m=self.model
        self.assertEqual(m['schema'],'shared_epl_mls_math_glm_v1')
        self.assertEqual(m['fit_support'],{'train':90,'validation':35,'test':35})
        self.assertFalse(m['identity_features_in_numeric_training'])
        self.assertEqual(set(m['coefficients']),{'start','shots','conversion'})
        for task,betas in m['coefficients'].items():
            self.assertEqual(len(betas),1+2*len(FEATURE_NAMES))
            self.assertTrue(all(math.isfinite(b) for b in betas))

    def test_scoring_is_deterministic_and_probabilistically_valid(self):
        first=score(self.model,self.rows[:20])
        self.assertEqual(first,score(self.model,self.rows[:20]))
        for r in first:
            self.assertTrue(0<r['p_start_unprojected']<1)
            self.assertTrue(0<r['p_goal_per_shot_given_playing']<1)
            self.assertGreater(r['shot_attempts_per90_given_playing'],0)

    def test_proper_scores_available_all_splits(self):
        for split in ('train','validation','test'):
            self.assertTrue(all(math.isfinite(v) for v in self.model['loss'][split].values()))

    def test_future_or_overlapping_training_fails(self):
        rows=fixtures()
        rows[0]['feature_cutoff_utc']='2026-01-01T00:00:00Z'
        with self.assertRaisesRegex(ValueError,'not earlier'):
            validate_splits(rows)
        rows=fixtures()
        rows[0]['date']=rows[-1]['date']
        with self.assertRaisesRegex(ValueError,'overlap'):
            validate_splits(rows)

    def test_corrupt_goals_greater_than_shots_rejected(self):
        rows=fixtures()
        rows[1]['label']['goals']=8
        with self.assertRaisesRegex(ValueError,'conservation'):
            fit(rows)

    def test_nonexistent_feature_names_cannot_be_fitted(self):
        rows=fixtures()
        rows[0]['features']['player_identity_rank']=100
        with self.assertRaisesRegex(ValueError,'frozen feature'):
            fit(rows)

    def test_single_math_model_accepts_the_full_248_column_contract(self):
        keys=tuple(f"canonical_{i:03d}" for i in range(248))
        rows=fixtures()
        for row in rows:
            row["features"]={k:(row["features"].get("sofascore_rating")
                                if j==7 else None)
                             for j,k in enumerate(keys)}
        model=fit(rows,penalty=1.0,feature_names=keys)
        self.assertEqual(model["feature_contract"],"unified_epl_mls_248_v1")
        self.assertEqual(model["feature_names"],list(keys))
        self.assertEqual(len(score(model,rows[:5])),5)
        corrupt=rows[:5]
        corrupt[0]["features"]["unregistered_future_feature"]=3.0
        with self.assertRaisesRegex(ValueError,"frozen feature"):
            score(model,corrupt)


if __name__=='__main__':
    unittest.main()
