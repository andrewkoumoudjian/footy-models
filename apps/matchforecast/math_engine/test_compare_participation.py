"""Fail-closed paired-comparison contracts for historical research."""
import importlib.util
import unittest
from pathlib import Path

P = Path(__file__).with_name('compare_participation.py')
spec = importlib.util.spec_from_file_location('math_compare_participation', P)
comparison = importlib.util.module_from_spec(spec)
spec.loader.exec_module(comparison)


def example():
    metrics = {key: 0.1 for key in comparison.TARGETS}
    return {
        'status':'exploratory_inspected_holdout_NOT_PROMOTABLE',
        'fixtures':[{'match_id':'1','date':'2016-02-13',**metrics}],
        'paths_per_match':2500,'seed':42,'source_fixture_sha256':'abc',
        'engine_binary_sha256':'def',
        'mean_losses':{
            'mean_simulated_home_substitutions':2,
            'mean_simulated_away_substitutions':2,
        }
    }


class ComparisonContracts(unittest.TestCase):
    def test_identical_fixture_sample_compares(self):
        a=example()
        b=example()
        b['fixtures'][0]['minutes_engine']=0.07
        result=comparison.compare(a,{'trial':b})
        self.assertEqual(result['fixtures'],1)
        self.assertAlmostEqual(
            result['results']['trial']['minutes_engine']['challenger_minus_baseline'],-0.03)

    def test_target_date_mismatch_rejected(self):
        a=example();b=example()
        b['fixtures'][0]['date']='2016-02-14'
        with self.assertRaisesRegex(ValueError,'fixture sample'):
            comparison.compare(a,{'trial':b})

    def test_changes_in_seed_or_heldout_identity_rejected(self):
        a=example();b=example()
        b['seed']=43
        with self.assertRaisesRegex(ValueError,'seed'):
            comparison.compare(a,{'trial':b})

    def test_unlabeled_promotion_claim_rejected(self):
        a=example();a['status']='validated_oos'
        with self.assertRaisesRegex(ValueError,'retrospective'):
            comparison.compare(a,{'trial':example()})


if __name__=='__main__':
    unittest.main()
