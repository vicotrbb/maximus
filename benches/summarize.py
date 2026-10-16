#!/usr/bin/env python3
"""Produce compact, auditable benchmark tables from the completed qualification campaign."""
import csv, json, pathlib, statistics
ROOT = pathlib.Path(__file__).resolve().parents[1]
RAW, OUT = ROOT / 'artifacts/bench', ROOT / 'benches/results'
CASES = {'workloads': 69, 'arrival': 15, 'scaling': 15, 'tls-h1': 18, 'tls-h2': 18, 'sustained': 1}
FIELDS = ['rps', 'p50_ms', 'p99_ms', 'p999_ms', 'cpu_us_per_request', 'server_cpu_percent', 'client_cpu_percent', 'rss_kib', 'peak_rss_kib']

def main():
    OUT.mkdir(exist_ok=True)
    all_rows, tables = [], []
    for case, expected in CASES.items():
        rows = list(csv.DictReader((RAW / f'qualified-{case}.csv').open()))
        assert len(rows) == expected, f'{case}: {len(rows)}/{expected} trials complete'
        for row in rows:
            row['campaign'] = case
            all_rows.append(row)
        keys = sorted({(r['engine'], r['path']) for r in rows})
        table = [f'## {case}', '', 'Median of independent trials; tails are medians of per-trial percentiles, not pooled percentiles.', '',
                 '| Engine | Path | RPS | p50 ms | p99 ms | p99.9 ms | Server µs/req | Server CPU % | Client CPU % | RSS MiB | Failed trials |',
                 '| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
        for engine, path in keys:
            group = [r for r in rows if (r['engine'], r['path']) == (engine, path)]
            med = {key: statistics.median(float(r[key]) for r in group) for key in FIELDS}
            failures = sum(int(r['exit']) != 0 for r in group)
            table.append(f'| {engine} | {path} | {med["rps"]:,.0f} | {med["p50_ms"]:.3f} | {med["p99_ms"]:.3f} | {med["p999_ms"]:.3f} | {med["cpu_us_per_request"]:.2f} | {med["server_cpu_percent"]:.1f} | {med["client_cpu_percent"]:.1f} | {med["rss_kib"]/1024:.2f} | {failures} |')
        tables.extend(table + [''])
    with (OUT/'trials.csv').open('w') as file:
        writer = csv.DictWriter(file, fieldnames=list(all_rows[0]))
        writer.writeheader(); writer.writerows(all_rows)
    (OUT/'tables.md').write_text('# Measured results\n\n' + '\n'.join(tables))
    environments = {case: json.loads((RAW / f'qualified-{case}-environment.json').read_text()) for case in CASES}
    for environment in environments.values():
        environment['background_container_count'] = len(environment.pop('running_containers').splitlines())
    (OUT/'environment.json').write_text(json.dumps(environments, indent=2) + '\n')
    errors = sum(int(r['failures']) + int(r['checks_failed']) for r in all_rows)
    dropped = sum(int(r['dropped']) for r in all_rows)
    print(f'{len(all_rows)} trials; response/check failures={errors}; dropped iterations={dropped}')

if __name__ == '__main__': main()
