"""Mechanism variants must share EXACT bytes of their source inputs and binary."""
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from summarize_frozen_pl_ablation import compare
from run_frozen_pl_research import sha256


class FrozenMechanismComparisonTest(unittest.TestCase):
    def setUp(self):
        root = tempfile.TemporaryDirectory()
        self.addCleanup(root.cleanup)
        self.root = Path(root.name)

    def archive(self, label, *, no_tactics=False, red_probability=None, binary=b"binary-v1"):
        directory = self.root / label
        directory.mkdir()
        files = {}
        source = {
            "binary": binary,
            "results": b"dated_team_features",
            "players": b"dated_player_features",
            "baseline": b"dated_team_baseline",
        }
        for name, content in source.items():
            path = directory / f"{name}.bin"
            path.write_bytes(content)
            files[name] = path.name
        forecast = {
            "as_of": "2026-10-10",
            "fixtures": [{
                "date": "2026-10-11", "home": "Liverpool", "away": "Man City",
                "paths": 12, "seed": 44,
                "home_win": 0.25, "draw": 0.25, "away_win": 0.50,
                "expected_home_goals": 1.0, "expected_away_goals": 1.5,
                "15_minute_windows": [
                    {
                        "expected_home_shots": 2.0,
                        "expected_away_shots": 2.0 + 0.1 * i,
                        "expected_home_goals": 0.2,
                        "expected_away_goals": 0.3,
                        "p_red_card": 0.0,
                        "p_substitution": 0.0,
                    }
                    for i in range(6)
                ],
            }],
        }
        path = directory / "forecast_1.json"
        path.write_text(json.dumps(forecast))
        files["forecast_1"] = path.name
        manifest = {
            "contract": "frozen_exact_binary_pre_match_research_v1",
            "cutoff": "2026-10-10",
            "forecast_date": "2026-10-11",
            "fixtures": [["2026-10-11", "Liverpool", "Man City"]],
            "paths": 12,
            "seed": 42,
            "repeat": 1,
            "variant": label,
            "research_flags": {
                "no_tactical_subs": no_tactics,
                "red_probability_per90": red_probability,
            },
            "archived_files": files,
            "sha256": {name: sha256(directory / filename) for name, filename in files.items()},
        }
        (directory / "manifest.json").write_text(json.dumps(manifest))
        return directory

    def test_comparison_accepts_identical_binary_inputs_only(self):
        normal = self.archive("normal")
        no_tactics = self.archive("no_tactics", no_tactics=True)
        report = compare([normal, no_tactics])
        self.assertEqual(report["paths_per_variant"], 12)
        self.assertEqual(len(report["variants"]), 2)
        self.assertEqual(report["variants"][0]["peak_relative_city_pressure_bin"], 5)
        self.assertIn("binary", report["shared_input_and_exact_executable_sha256"])

    def test_rejects_changed_binary_and_duplicate_variants(self):
        normal = self.archive("normal")
        bad_binary = self.archive("different", no_tactics=True, binary=b"binary-v2")
        with self.assertRaisesRegex(ValueError, "do not match: binary"):
            compare([normal, bad_binary])
        same_variant = self.archive("repeat")
        with self.assertRaisesRegex(ValueError, "repeated variant"):
            compare([normal, same_variant])

    def test_rejects_tampered_output(self):
        normal = self.archive("normal")
        no_tactics = self.archive("no_tactics", no_tactics=True)
        (no_tactics / "forecast_1.json").write_text("{}")
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            compare([normal, no_tactics])


if __name__ == "__main__":
    unittest.main()
