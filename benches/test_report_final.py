"""A report must not promote a failed or interrupted campaign into qualification."""

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest
from workloads import SOCKET_CASES, response_keys

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
            self.assertTrue(output.exists(), result.stderr)
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
                    "generator": "k6",
                    "requests": 100,
                    "iterations_started": 100,
                    "iterations_completed": 100,
                    "failures": 0,
                    "checks_failed": 0,
                    "network_errors": 0,
                    "dropped": 0,
                    "exit": 0,
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

    def echo_pair(self, size):
        trials = [self.trial("baseline"), self.trial("candidate")]
        for trial in trials:
            trial["case"] = 6 if size == 1024 else 7
            row = dict(trial["rows"][0], path="/echo", body_bytes=size)
            trial["rows"] = [row]
            if trial["variant"] == "candidate":
                trial["command"][-1] = "maximus,hyper,axum"
                trial["rows"] += [dict(row, engine=engine) for engine in ("hyper", "axum")]
        return trials

    def test_echo_cases_preserve_matching_payload_and_all_peers(self):
        for size, case in [(1024, 6), (131072, 7)]:
            with self.subTest(size=size):
                self.assertGreater(len(SOCKET_CASES), case, "echo workload is missing")
                self.assertEqual(
                    response_keys(case, ["maximus", "hyper", "axum"]),
                    {("plain", "/echo", size, engine) for engine in ("maximus", "hyper", "axum")},
                )
                trials = self.echo_pair(size)
                result, report = self.report(trials, cases=str(case))
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertTrue(report["qualified_input"])
                self.assertEqual(report["trials"], trials)

    def test_echo_cases_reject_wrong_size_missing_peer_and_duplicates(self):
        for defect in ("size", "peer", "duplicate", "completion"):
            with self.subTest(defect=defect):
                trials = self.echo_pair(131072)
                if defect == "size":
                    trials[1]["rows"][0]["body_bytes"] = 1024
                elif defect == "peer":
                    trials[1]["rows"].pop()
                elif defect == "duplicate":
                    trials[1]["rows"].append(trials[1]["rows"][0])
                else:
                    trials[1]["passed"] = False
                    trials[1]["exit"] = 1
                result, report = self.report(trials, cases="7")
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(report["qualified_input"])
                self.assertEqual(report["trials"], trials)
                self.assertEqual(report["paired"], {})

    def test_raw_failures_and_incomplete_iterations_override_driver_pass(self):
        for metric, value in [
            ("failures", 1), ("checks_failed", 1), ("network_errors", 64),
            ("dropped", 1), ("exit", 1), ("requests", 0),
            ("requests", "nan"), ("iterations_started", 101),
            ("iterations_completed", 99),
        ]:
            with self.subTest(metric=metric, value=value):
                trials = [self.trial("baseline"), self.trial("candidate")]
                trials[1]["rows"][0][metric] = value
                result, report = self.report(trials)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(report["qualified_input"])
                self.assertEqual(report["paired"], {})
                self.assertEqual(report["trials"], trials)

    def test_explicit_engine_contract_supports_native_commands(self):
        trials = [self.trial("baseline"), self.trial("candidate")]
        for trial in trials:
            trial["engines"] = ["maximus"]
            trial["command"] = ["run-native.py", "--backend", "poll"]
        result, report = self.report(trials)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(report["qualified_input"])


if __name__ == "__main__":
    unittest.main()
