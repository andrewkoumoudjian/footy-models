"""Model code, inputs and binary identity must be verified, never inferred."""
import argparse
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from run_frozen_pl_research import (
    create_experiment, preflight, sha256, verify_archive,
)


class FrozenResearchTest(unittest.TestCase):
    def setUp(self):
        root = tempfile.TemporaryDirectory()
        self.addCleanup(root.cleanup)
        self.root = Path(root.name)
        self.binary = self.root / "model"
        self.binary.write_text(
            "#!/usr/bin/env python3\n"
            "import json,sys\n"
            "a=sys.argv\n"
            "value=lambda flag:a[a.index(flag)+1]\n"
            "f={'date':'2026-10-11','home':'Liverpool','away':'Man City',"
            "'paths':int(value('--paths'))}\n"
            "with open(value('--output'),'w') as output:\n"
            " json.dump({'as_of':value('--snapshot-as-of'),'fixtures':[f]}, output)\n"
        )
        self.binary.chmod(self.binary.stat().st_mode | 0o111)
        self.inputs = {
            name: self.root / f"{name}.txt"
            for name in ("results", "players", "baseline")
        }
        self.inputs["results"].write_text("date,team\n2026-01-01,A\n")
        self.inputs["players"].write_text("name,club\nPlayer,Liverpool\n")
        self.inputs["baseline"].write_text(json.dumps({
            "fixtures": [{"date": "2026-10-11", "home": "Liverpool", "away": "Man City"}],
        }))
        self.output = self.root / "run"

    def args(self):
        return argparse.Namespace(
            output_dir=self.output, binary=self.binary, **self.inputs,
            as_of="2026-10-11", snapshot_as_of="2026-10-10",
            paths=2, seed=42, only="Liverpool", repeat=2,
            no_tactical_subs=False, red_probability=None,
        )

    def test_repeat_verified_and_executable_retained(self):
        manifest = create_experiment(self.args())
        self.assertEqual(manifest["repeat"], 2)
        self.assertEqual(manifest["sha256"]["forecast_1"], manifest["sha256"]["forecast_2"])
        self.assertEqual(manifest["sha256"]["binary"], sha256(self.binary))
        self.assertTrue((self.output / manifest["archived_files"]["binary"]).stat().st_mode & 0o111)
        self.assertEqual(verify_archive(self.output)["contract"],
                         "frozen_exact_binary_pre_match_research_v1")
        with self.assertRaises(FileExistsError):
            create_experiment(self.args())
        (self.output / "forecast_1.json").write_text("{}")
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            verify_archive(self.output)

    def test_asof_cutoff_and_missing_data_fail_closed(self):
        args = self.args()
        with self.assertRaisesRegex(ValueError, "cutoff"):
            preflight(
                binary=self.binary, inputs=self.inputs,
                as_of="2026-10-10", snapshot_as_of="2026-10-10",
                only="Liverpool", paths=2, seed=42, repeat=2,
            )
        args.only = "Juventus"
        with self.assertRaisesRegex(ValueError, "no matching"):
            create_experiment(args)
        self.assertFalse(self.output.exists())

    def test_identical_inputs_independent_of_future_label(self):
        manifest = create_experiment(self.args())
        self.assertNotIn("result", manifest["archived_files"])
        self.assertNotIn("liverpool-city-actual", str(manifest))
        self.assertEqual(manifest["cutoff"], "2026-10-10")
        self.assertTrue(manifest["working_tree_tracked_dirty_at_run"] in (True, False))


if __name__ == "__main__":
    unittest.main()
