"""A report must not promote a failed or interrupted campaign into qualification."""

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

REPORT = pathlib.Path(__file__).with_name("report-final.py")


class QualificationTests(unittest.TestCase):
    def report(self, trials, cases="1"):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source = root / "campaign.json"
            output = root / "report.json"
            source.write_text(json.dumps(trials))
            result = subprocess.run(
                [
                    sys.executable,
                    str(REPORT),
                    "--campaign",
                    str(source),
                    "--output",
                    str(output),
                    "--expected-repeats",
                    "1",
                    "--expected-cases",
                    cases,
                ],
                check=False,
                capture_output=True,
                text=True,
            )
            return result, json.loads(output.read_text())

    def trial(self, variant, passed=True):
        return {
            "repeat": 0,
            "case": 1,
            "variant": variant,
            "label": variant,
            "passed": passed,
            "exit": 0 if passed else 1,
            "command": ["run.py", "--engines", "maximus"],
            "rows": [
                {
                    "protocol": "plain",
                    "path": "/sse",
                    "body_bytes": 0,
                    "engine": "maximus",
                    "rps": 100,
                    "p50_ms": 1,
                    "p99_ms": 2,
                    "p999_ms": 3,
                    "cpu_us_per_request": 10,
                    "rss_kib": 1000,
                    "peak_rss_kib": 1000,
                    "client_cpu_percent": 100,
                    "server_cpu_percent": 50,
                }
            ],
        }

    def test_complete_passing_pair_reports_both_observations(self):
        trials = [self.trial("baseline"), self.trial("candidate")]
        result, report = self.report(trials)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(report["qualified_input"])
        self.assertEqual(report["trials"], trials)
        self.assertEqual(
            report["paired"]["plain|/sse|0|maximus"]["rps"]["paired_percent"], [0.0]
        )

    def test_missing_response_row_is_rejected(self):
        trials = [self.trial("baseline"), self.trial("candidate")]
        trials[1]["rows"] = []
        result, report = self.report(trials)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(report["qualified_input"])

    def test_same_missing_path_in_both_variants_is_rejected(self):
        trials = [self.trial("baseline"), self.trial("candidate")]
        for trial in trials:
            trial["case"] = 0
            row = trial["rows"][0]
            # Both variants lack /error, while every observed key is paired.
            trial["rows"] = [dict(row, path="/plaintext"), dict(row, path="/users/42")]
        result, report = self.report(trials, cases="0")
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(report["qualified_input"])
        self.assertEqual(len(report["diagnostics"]["response_contract"]), 2)
        self.assertEqual(report["trials"], trials)

    def test_missing_pair_is_preserved_and_rejected(self):
        trials = [self.trial("baseline")]
        result, report = self.report(trials)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(report["qualified_input"])
        self.assertEqual(report["trials"], trials)
        self.assertEqual(report["paired"], {})

    def test_failure_is_not_hidden_by_a_complete_pair(self):
        trials = [self.trial("baseline"), self.trial("candidate", False)]
        result, report = self.report(trials)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(report["diagnostics"]["failed_trials"], ["candidate"])
        self.assertEqual(report["trials"], trials)

    def test_duplicate_trial_is_rejected(self):
        trials = [
            self.trial("baseline"),
            self.trial("candidate"),
            self.trial("candidate"),
        ]
        result, report = self.report(trials)
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(report["diagnostics"]["duplicate_trials"])


if __name__ == "__main__":
    unittest.main()
