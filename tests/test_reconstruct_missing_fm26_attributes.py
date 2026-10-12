"""FM26 source-fitted priors, never fabricated actual mental ratings."""
import sys
import unittest
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/"scripts"))
from reconstruct_missing_fm26_attributes import (
    ca_position_mixture, curve_moments, position_class,
)


def fake_curves():
    return {
        klass:{
            "Composure":[(float(base), 2.0)]*201,
            "Decisions":[(float(base), 3.0)]*201,
            "Handling":[(float(base), 2.0)]*201,
        }
        for klass,base in (("GK",14),("DF",11),("MF",12),("FW",13))
    }


class FMReconstructionTests(unittest.TestCase):
    def test_role_and_ca_conditional_moments_with_source_uncertainty(self):
        o=curve_moments(fake_curves(),"DF",150)
        self.assertEqual(o["Composure"]["mean"],11)
        self.assertEqual(o["Composure"]["std"],2)
        self.assertEqual(o["Handling"]["mean"],0)
        self.assertLess(o["Composure"]["p10"],o["Composure"]["mean"])
        self.assertGreater(o["Composure"]["p90"],o["Composure"]["mean"])

    def test_unknown_role_uses_uncertain_mixture_not_guessed_midfielder(self):
        weights={"GK":0.09,"DF":0.19,"MF":0.51,"FW":0.21}
        mixed=ca_position_mixture(fake_curves(),150,weights)
        # Mixture mean is not rounded into a fictional integer FM rating.
        self.assertGreater(mixed["Composure"]["mean"],11)
        self.assertLess(mixed["Composure"]["mean"],13)
        self.assertGreater(mixed["Composure"]["std"],2.0)
        self.assertLess(mixed["Handling"]["mean"],2)
        self.assertGreater(mixed["Handling"]["std"],0)

    def test_missing_ca_is_not_silently_assigned_or_overridden(self):
        with self.assertRaises(ValueError):
            curve_moments(fake_curves(),"GK",0)
        with self.assertRaises(ValueError):
            curve_moments(fake_curves(),"MF",201)

    def test_sortitoutsi_role_class(self):
        self.assertEqual(position_class("GK"),"GK")
        self.assertEqual(position_class("D C, DM"),"DF")
        self.assertEqual(position_class("M C"),"MF")
        self.assertEqual(position_class("ST"),"FW")
        self.assertIsNone(position_class(None))


if __name__=="__main__":
    unittest.main()
