"""Fail-closed accuracy and OOS-version compatibility checks."""
import copy
import json
import sys
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'scripts'))
from check_forecast_release import audit,same_engine_oos_ok
from calibrate_assist_prior_oos import posterior,log_loss
from run_pl_pipeline import model_code_fingerprint,model_code_files

class ForecastTargetsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract=json.loads((ROOT/'configs/forecast_targets_v1.json').read_text())
        cls.snapshot=json.loads((ROOT/'artifacts/premier-league-2026-10-11/player_paths.json').read_text())
        cls.events=json.loads((ROOT/'artifacts/validation/premier-league-events-2015-16-oos.json').read_text())
        cls.assists=json.loads((ROOT/'artifacts/validation/primary-assist-prior-2015-16-oos.json').read_text())

    def test_structured_scope_and_unique_ids(self):
        keys=[t['id'] for t in self.contract['targets']]
        self.assertEqual(len(keys),len(set(keys)))
        self.assertIn('match_wdl',keys)
        self.assertIn('player_goal',keys)
        self.assertIn('time_goal_1min',keys)
        self.assertIn('attacking_momentum_index',keys)
        self.assertGreaterEqual(self.contract['minimum_independent_paths_for_public_result_probability'],2400)

    def test_no_historical_model_backdoor_to_fm_release(self):
        results=audit(self.contract,self.snapshot,self.events,self.assists)
        self.assertEqual(results['path_count_min'],72)
        self.assertFalse(results['all_match_targets_calibrated'])
        self.assertFalse(results['matching_engine_oos_valid'])
        self.assertTrue(.10<results['worst_case_mc_halfwidth_95']<.13)
        self.assertEqual(next(t for t in results['targets'] if t['id']=='attacking_momentum_index')['publication_status'],'descriptive_only')
        self.assertEqual(next(t for t in results['targets'] if t['id']=='player_goal')['publication_status'],'unavailable')
        self.assertFalse(any(t['publication_status']=='calibrated_for_scored_target' for t in results['targets']))

    def test_matching_simulator_prospective_evidence_can_upgrade_one_target(self):
        model=copy.deepcopy(self.snapshot)
        model['model_code_sha256']='a'*64
        for f in model['fixtures']:f['paths']=2800
        report={'schema_version':1,'model_code_sha256':'a'*64,'model':model['model'],
            'protocol':'same_engine_walk_forward_frozen_pre_kickoff','fit_end_date':'2025-05-15',
            'oos_start_date':'2025-05-20','oos_end_date':'2026-08-01',
            'tested_without_selection_or_tuning':True,'same_fixture_engine_and_output_contract':True,
            'targets':{'match_wdl':{'distinct_oos_fixtures':250,'primary_score':'multiclass_log_loss',
                'label_coverage':1.0,'matched_day_bootstrap_95':[-.08,-.012]}}}
        self.assertTrue(same_engine_oos_ok(model,report))
        result=audit(self.contract,model,matching_oos=report)
        self.assertEqual(next(t for t in result['targets'] if t['id']=='match_wdl')['publication_status'],
                         'calibrated_for_scored_target')
        self.assertFalse(result['all_match_targets_calibrated'])
        report['tested_without_selection_or_tuning']=False
        self.assertFalse(same_engine_oos_ok(model,report))
        self.assertNotEqual(next(t for t in audit(self.contract,model,matching_oos=report)['targets'] if t['id']=='match_wdl')['publication_status'],
                            'calibrated_for_scored_target')

    def test_new_snapshot_model_code_fingerprint_covers_real_source(self):
        files=model_code_files()
        self.assertGreater(len(files),40)
        self.assertTrue(all((ROOT/file).is_file() for file in files))
        self.assertIn('crates/match/src/second_resolved_mc.rs',files)
        self.assertIn('apps/matchforecast/src/driver.rs',files)
        self.assertEqual(len(model_code_fingerprint()),64)
        self.assertEqual(model_code_fingerprint(),model_code_fingerprint())

    def test_assist_posterior_fails_against_prior_reference(self):
        a=self.assists
        self.assertFalse(a['promotion']['approve_historical_prior'])
        self.assertLess(a['training_only_beta_2_2_posterior'],a['holdout_observed_ratio'])
        self.assertGreater(a['mean_binary_log_loss_per_goal']['historical_posterior'],
                           a['mean_binary_log_loss_per_goal']['preexisting_0_68'])
        self.assertAlmostEqual(posterior([{'home':'a','away':'b','teams':{'a':{'counts':{'goals':1,'assists':1}},
            'b':{'counts':{'goals':0,'assists':0}}}}]),(2+1)/(4+1))
        self.assertLess(log_loss(.7,1,1),log_loss(.5,1,1))

if __name__=='__main__':unittest.main()
