#!/usr/bin/env python3
"""Runner metadata controls; no containers or timed workloads are launched."""
import importlib.util
import json
import pathlib
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('benchmark_run', pathlib.Path(__file__).with_name('run.py'))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class ExecutableMetadata(unittest.TestCase):
    def test_selected_engines_determine_executable_provenance(self):
        for engines in ['nginx', 'nginx,nginx', 'nginx,hyper', 'maximus,axum']:
            with self.subTest(engines=engines), tempfile.TemporaryDirectory() as directory:
                binary_calls = []

                def capture(*args):
                    if 'sha256sum' in args:
                        binary_calls.append(args)
                        if set(engines.split(',')) == {'nginx'}:
                            raise FileNotFoundError('comparison executable is absent')
                        return 'digest  /target/release/maximus-comparison'
                    return 'metadata'

                with patch.object(runner, 'OUT', pathlib.Path(directory)), patch.object(runner, 'capture', capture), patch('sys.argv', ['run.py', '--engines', engines, '--repeats', '0', '--label', 'metadata']):
                    runner.main()
                data = json.loads((pathlib.Path(directory) / 'metadata-environment.json').read_text())
                needs_binary = set(engines.split(',')) != {'nginx'}
                self.assertEqual(len(binary_calls), int(needs_binary))
                self.assertEqual(data['binary_sha256'], 'digest  /target/release/maximus-comparison' if needs_binary else None)


if __name__ == '__main__':
    unittest.main()
