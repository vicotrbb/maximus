#!/usr/bin/env python3
"""Audit and summarize the approved local Docker/k6 campaign without dropping trials."""
import csv
import hashlib
import json
import math
import pathlib
import statistics

ROOT = pathlib.Path(__file__).resolve().parents[1]
CAMPAIGN = ROOT / 'artifacts/performance-2026-10-20/local-k6-approved'
RESULTS = ROOT / 'benches/results'
VARIANTS = ['original', 'baseline', 'borrowed-errors', 'h1-empty-tracking', 'h2-borrowed-timer']
METRICS = ['rps', 'p50_ms', 'p99_ms', 'p999_ms', 'cpu_us_per_request',
           'rss_kib', 'peak_rss_kib', 'server_cpu_percent', 'client_cpu_percent']


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    outcomes = json.loads((CAMPAIGN / 'outcomes.json').read_text())
    plan = json.loads((CAMPAIGN / 'plan.json').read_text())
    frozen = json.loads((CAMPAIGN / 'frozen-binaries.json').read_text())
    environment = json.loads((CAMPAIGN / 'environment.json').read_text())
    footprints = json.loads((CAMPAIGN / 'footprints.json').read_text())
    allocations = json.loads((CAMPAIGN / 'allocations.json').read_text())
    manifest = json.loads((CAMPAIGN / 'source-manifest.json').read_text())
    for relative, expected in environment['scripts'].items():
        assert digest(ROOT / relative) == expected, relative
    for variant in VARIANTS:
        source_hashes = json.loads((CAMPAIGN / f'{variant}-source-hashes.json').read_text())
        expected = dict(manifest['files_sha256'])
        if variant == 'original':
            ref = manifest['original_performance_reference']
            expected[ref['target']] = ref['sha256']
        elif variant in manifest['variants']:
            expected.update(manifest['variants'][variant])
        assert source_hashes == expected, variant
        assert all(digest(CAMPAIGN / variant / path) == value for path, value in expected.items()), variant
        build_log = (CAMPAIGN / f'{variant}-build.log').read_text()
        for name, value in frozen[variant].items():
            assert f'{value}  /target/local-k6-approved-{variant}/{name}' in build_log, (variant, name)
    expected_trials = {}
    for repeat in range(2):
        for case, (protocol, path) in enumerate([('plain', '/plaintext'), ('h2', '/error')]):
            for engine in ('maximus', 'hyper', 'axum'):
                expected_trials[f'local-k6-aa-{repeat}-{case}-{engine}'] = ('baseline', engine, protocol, path, 1024, 'aa', repeat, case)
    for repeat in range(plan['repeats']):
        for case, (protocol, path, size, engine, variants) in enumerate(plan['cases']):
            for variant, target in [(v, engine) for v in variants] + [('baseline','hyper'),('baseline','axum')]:
                expected_trials[f'local-k6-main-{repeat}-{case}-{variant}-{target}'] = (variant,target,protocol,path,size,'main',repeat,case)
    assert len({o['label'] for o in outcomes}) == len(outcomes), 'duplicate trial labels'
    audit_failures = []
    identities = []
    rows = []
    warmup_requests = 0
    certificates = set()
    for attempt in outcomes:
        label = attempt['label']
        assert tuple(attempt[k] for k in ('variant','engine','protocol','path','size','phase','repeat','case')) == expected_trials[label], label
        raw = ROOT / 'artifacts/bench' / f'{label}-environment.json'
        env = json.loads(raw.read_text())
        assert env['args']['label'] == label and env['source_commit'] == manifest['original_head'], label
        assert env['args']['binary'] == f'/target/local-k6-approved-{attempt["variant"]}/maximus-comparison', label
        assert env['binary_sha256'].split()[0] == frozen[attempt['variant']]['maximus-comparison'], label
        assert env['image_ids']['rust:1.96-bookworm'] == environment['rust_image_id'], label
        assert env['image_ids']['grafana/k6:1.6.0'] == environment['k6_image_id'], label
        # uname -a includes the disposable metadata container's hostname.
        kernel_identity = lambda value: value.split()[:1] + value.split()[2:]
        assert kernel_identity(env['kernel']) == kernel_identity(environment['kernel']) and env['cpus'] == '3', label
        assert env['workload_sha256'] == environment['scripts']['benches/load.js'], label
        assert env['args']['generator'] == 'k6' and env['args']['client_cpus'] == '1,2', label
        assert env['args']['workers'] == 1 and env['args']['vus'] == 64, label
        assert env['args']['duration'] == plan['duration'], label
        assert env['args']['protocol'] == attempt['protocol'], label
        assert env['args']['engines'] == attempt['engine'] and env['args']['paths'] == attempt['path'], label
        assert env['args']['body_bytes'] == attempt['size'] and env['args']['rates'] == '0' and env['args']['repeats'] == 1, label
        csv_path = ROOT / 'artifacts/bench' / f'{label}.csv'
        actual_rows = list(csv.DictReader(csv_path.open())) if csv_path.exists() else []
        assert actual_rows == attempt['rows'], label
        if attempt['protocol'] != 'plain':
            certificates.add(env['certificate_sha256'])
        identities.append({'label': label, 'environment_sha256': digest(raw), 'binary_sha256': env['binary_sha256'].split()[0]})
        for item in attempt['rows']:
            row = {k: float(v) if k not in ('engine', 'path', 'protocol', 'generator') else v for k, v in item.items()}
            assert all(math.isfinite(row[m]) and row[m] >= 0 for m in METRICS), label
            row.update({k: attempt[k] for k in ('label', 'variant', 'phase', 'repeat', 'case', 'passed')})
            assert item['engine'] == attempt['engine'] and item['path'] == attempt['path'] and item['protocol'] == attempt['protocol'], label
            assert int(item['body_bytes']) == (attempt['size'] if attempt['path']=='/echo' else 0), label
            assert int(item['workers']) == 1 and int(item['offered_rps']) == 0 and int(item['repeat']) == 0, label
            rows.append(row)
            key = f'{label}-{item["engine"]}-{item["path"].strip("/").replace("/", "-")}-w1-r0-n0'
            summary_path = ROOT / 'artifacts/bench' / f'{key}.json'
            summary = json.loads(summary_path.read_text())
            warmup = json.loads((ROOT / 'artifacts/bench' / f'{key}-warmup.json').read_text())
            for phase, data in [('measured', summary), ('warmup', warmup)]:
                counts = [data['metrics'][m]['values']['count'] for m in ('http_reqs', 'iteration_starts', 'iterations')]
                valid = all(isinstance(v,(int,float)) and not isinstance(v,bool) and math.isfinite(v) and v>0 and int(v)==v for v in counts) and len(set(counts))==1
                metrics = data['metrics']
                for metric, field in [('checks','fails'),('http_req_failed','passes'),('dropped_iterations','count'),('network_errors','count')]:
                    valid = valid and metrics.get(metric,{}).get('values',{}).get(field,0)==0
                valid = valid and metrics['checks']['values']['passes']==counts[0]
                if not valid:
                    audit_failures.append(f'{label}:{phase}:raw-correctness')
                if phase=='measured':
                    assert counts[0]==row['requests'], label
                    for metric, field, column in [('http_reqs','rate','rps'),('http_req_duration','med','p50_ms'),('http_req_duration','p(99)','p99_ms'),('http_req_duration','p(99.9)','p999_ms')]:
                        assert metrics[metric]['values'][field] == row[column], (label,column)
            warmup_requests += warmup['metrics']['http_reqs']['values']['count']
            identities[-1]['summary_sha256'] = digest(summary_path)
    assert len(certificates) <= 1, 'TLS trials used different certificates'
    main_rows = [r for r in rows if r['phase'] == 'main']
    paired = []
    for variant in VARIANTS[2:]:
        for reference in VARIANTS[:2]:
            for case in range(len(plan['cases'])):
                changes = {m: [] for m in METRICS}
                for repeat in range(plan['repeats']):
                    select = lambda v: [r for r in main_rows if r['variant'] == v and r['engine'] not in ('hyper', 'axum') and r['case'] == case and r['repeat'] == repeat]
                    candidate, baseline = select(variant), select(reference)
                    if len(candidate) != 1 or len(baseline) != 1:
                        continue
                    for metric in METRICS:
                        changes[metric].append(100 * (candidate[0][metric] / baseline[0][metric] - 1))
                if changes['rps']:
                    paired.append(dict(variant=variant, reference=reference, case=case,
                                       metrics={m: dict(median=statistics.median(v), minimum=min(v), maximum=max(v), samples=v) for m, v in changes.items()}))
    aa = []
    for case in range(2):
        for engine in ('maximus', 'hyper', 'axum'):
            selected = sorted([r for r in rows if r['phase'] == 'aa' and r['case'] == case and r['engine'] == engine], key=lambda r: r['repeat'])
            if len(selected) == 2:
                aa.append(dict(case=case, engine=engine, percent_change={m: 100*(selected[1][m]/selected[0][m]-1) for m in METRICS}))
    aggregate = []
    for case in range(len(plan['cases'])):
        for variant, engine in [('baseline', plan['cases'][case][3]), ('baseline', 'hyper'), ('baseline', 'axum')]:
            selected = [r for r in main_rows if r['case'] == case and r['variant'] == variant and r['engine'] == engine]
            if selected:
                aggregate.append(dict(case=case, engine=engine, count=len(selected), metrics={m: statistics.median(r[m] for r in selected) for m in METRICS}))
    complete = {o['label'] for o in outcomes} == set(expected_trials) and len(rows) == len(expected_trials)
    evidence = dict(scope='Exploratory local Docker/k6 observations, not performance acceptance.', complete=complete,
                    attempts=len(outcomes), rows=len(rows), failed_attempts=[o['label'] for o in outcomes if not o['passed']], raw_correctness_failures=audit_failures,
                    measured_requests=sum(r['requests'] for r in rows), warmup_requests=warmup_requests,
                    plan=plan, environment=environment, frozen_binaries=frozen, identities=identities,
                    aa=aa, paired=paired, aggregate=aggregate, footprints=footprints, allocations=allocations,
                    build_log_sha256={v: digest(CAMPAIGN / f'{v}-build.log') for v in VARIANTS})
    RESULTS.mkdir(exist_ok=True)
    (RESULTS / 'local-docker-k6.json').write_text(json.dumps(evidence, indent=2) + '\n')
    if rows:
        with (RESULTS / 'local-docker-k6-trials.csv').open('w') as stream:
            writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
            writer.writeheader()
            writer.writerows(rows)
    lines = ['# Local Docker and k6 results', '',
             '**No performance optimization is accepted from these exploratory measurements.**', '',
             f'{len(outcomes)} attempts; {len(rows)} recorded observations; {len(evidence["failed_attempts"])} failed attempts. '
             f'{evidence["measured_requests"]:,.0f} measured requests and {warmup_requests:,.0f} warmup requests. '
             f'Campaign complete: {complete}; raw correctness audit failures: {len(audit_failures)}.', '',
             'Docker on this machine: Linux ARM64, 3 VM CPUs, approximately 4 GB RAM. '
             'Frozen Rust 1.96.1 builds; Hyper 1.12.0 and Axum 0.8.9 from the benchmark lockfile. '
             'One server worker on CPU 0; k6 on CPUs 1 and 2; 64 virtual users, '
             '3 s warmup and 12 s measured runs. Three planned main repetitions; two initial same-executable controls. '
             'The host is shared. All five independently built variants run every workload. '
             'Raw observations include failures, client CPU, wall-clock CPU intervals and sampled RSS. '
             'CPU accounting spans Docker client startup/teardown and sampling, not exactly the k6 duration. '
             'RSS is process memory; the sampled peak is not an exact high-water mark. '
             'Small-request trials show much higher client CPU utilization than server utilization. '
             'Their closed-loop request rates therefore do not establish maximum server capacity '
             'or a framework throughput ranking. CPU/request is a local measured cost under the recorded load.', '',
             'The original reference uses b5bb664 production source with the current shared harness. '
             'It retains the known rejected-CONNECT defect and is a valid-workload performance reference only. '
             'Separate exact-source correctness checks are documented in [candidate preparation](dedicated-qualification-preparation.md) and [root verification](performance-research-final.json). '
             'Hyper and Axum are comparison fixtures, not claims of complete feature parity.', '',
             '## Corrected baseline versus Hyper and Axum', '',
             '| Workload | Engine | n/3 | Median requests/s | Median p99 ms | Median CPU µs/request | Median RSS KiB |',
             '|---|---|---:|---:|---:|---:|---:|']
    for item in aggregate:
        protocol, path, size, _, _ = plan['cases'][item['case']]
        m = item['metrics']
        lines.append(f'| {protocol} {path} ({size if path=="/echo" else 0} B) | {item["engine"]} | {item["count"]}/3 | {m["rps"]:,.0f} | {m["p99_ms"]:.3f} | {m["cpu_us_per_request"]:.2f} | {m["rss_kib"]:,.0f} |')
    lines += ['', '## Candidate paired deltas against the corrected baseline', '',
              'Percent changes are candidate/reference − 1 for each repetition, then the median. '
              'Positive throughput is faster; negative CPU, latency and RSS is lower. '
              'These deltas are observations, not accepted gains. The JSON retains every sample, '
              'range and comparison against the original reference.', '',
              '| Candidate | Workload | n/3 | Requests/s Δ | CPU/request Δ | p99 Δ | p99.9 Δ | RSS Δ |',
              '|---|---|---:|---:|---:|---:|---:|---:|']
    for item in paired:
        if item['reference'] != 'baseline':
            continue
        protocol, path, *_ = plan['cases'][item['case']]
        cells = ' | '.join(f'{item["metrics"][m]["median"]:+.2f}%' for m in ['rps','cpu_us_per_request','p99_ms','p999_ms','rss_kib'])
        lines.append(f'| {item["variant"]} | {protocol} {path} | {len(item["metrics"]["rps"]["samples"])}/3 | {cells} |')
    lines += ['', '## Same-executable control variation', '',
              '| Case | Engine | Requests/s Δ | CPU/request Δ | p99 Δ | p99.9 Δ |',
              '|---|---|---:|---:|---:|---:|']
    for item in aa:
        cells = ' | '.join(f'{item["percent_change"][m]:+.2f}%' for m in ['rps','cpu_us_per_request','p99_ms','p999_ms'])
        lines.append(f'| {item["case"]} | {item["engine"]} | {cells} |')
    lines += ['', '## Linked footprint and allocation diagnostics', '',
              'These measurements ran before k6, in separate processes. Text/data/BSS are GNU `size` '
              'aggregates; file bytes and stripped bytes are retained in JSON. '
              'Every candidate increases text in at least one linked artifact versus the corrected baseline. '
              'All three stripped artifact file sizes remain unchanged across variants; ELF layout granularity '
              'means a smaller text aggregate need not shrink the executable file.', '',
              '| Source | Minimal hello text B | Conformance text B | Comparison text B |',
              '|---|---:|---:|---:|']
    for variant in VARIANTS:
        lines.append('| ' + variant + ' | ' + ' | '.join(f'{footprints[variant][name]["text"]:,}' for name in ['hello','conformance','maximus-comparison']) + ' |')
    lines += ['', '| Source | Static error allocations | Static error bytes | Owned error allocations | H1 keepalive allocations/request |',
              '|---|---:|---:|---:|---:|']
    for variant in VARIANTS:
        by_path = {r['path']:r for r in allocations[variant]}
        static, owned, h1 = (by_path[name] for name in ['static-error','owned-error','http1-keepalive-complete'])
        lines.append(f'| {variant} | {static["allocations_per_request"]} | {static["allocated_bytes_per_request"]} | {owned["allocations_per_request"]} | {h1["allocations_per_request"]} |')
    lines += ['', 'The borrowed-error candidate removes one allocation and 9 allocated bytes for the '
              'static error fixture; owned errors remain at three allocations. The empty-H1 candidate '
              'removes approximately one allocation in the full keepalive diagnostic. Fractional network '
              'counts include process-wide runtime activity; allocation reduction alone is not throughput '
              'acceptance. Diagnostic elapsed times are retained as raw data and are not treated as a timing campaign.']
    if complete:
        last = next(r for r in rows if r['label'] == 'local-k6-main-2-6-baseline-maximus')
        adverse_examples = []
        for variant, case in [('borrowed-errors', 4), ('h1-empty-tracking', 4), ('h2-borrowed-timer', 5)]:
            observation = next(p for p in paired if p['variant'] == variant and p['reference'] == 'baseline' and p['case'] == case)
            values = observation['metrics']
            adverse_examples.append(f'{variant} on {plan["cases"][case][0]} {plan["cases"][case][1]}: '
                                    f'requests/s {values["rps"]["median"]:+.2f}%, CPU/request {values["cpu_us_per_request"]["median"]:+.2f}%, '
                                    f'p99.9 {values["p999_ms"]["median"]:+.2f}%')
        lines += ['', '## Disposition and final-run anomaly', '',
                  'All 159 expected unique observations are present, with three samples in every main cell. '
                  f'The raw correctness audit records {len(audit_failures)} failures. '
                  'No performance candidate is promoted: concrete allocation reductions coexist with '
                  'linked-text growth and adverse or inconsistent local timing/resource observations.', '',
                  'Examples of paired median observations versus corrected baseline are: ' + '; '.join(adverse_examples) + '. '
                  'These are observed differences, not established causal effects of the patches. '
                  'The full tables and JSON retain favorable observations as well.', '',
                  f'The final unchanged corrected-baseline H2 upload trial recorded {last["rps"]:.2f} requests/s, '
                  f'p99 {last["p99_ms"]:.2f} ms, p99.9 {last["p999_ms"]:.2f} ms and a '
                  f'{last["cpu_interval_seconds"]:.2f} s CPU-accounting interval. '
                  'Every request still passed the response and iteration checks. This severe outlier '
                  'is retained in all aggregates and pairs; it inflates some apparent candidate upload '
                  'gains. The observations do not establish its cause. Together with the same-executable '
                  'variation and high client CPU, it prevents strong capacity or universal performance conclusions.', '']
    lines += ['', 'The campaign uses partially balanced source order and three samples per main cell. '
              'It cannot establish universal absence of regressions. Repeatable adverse observations, '
              'unstable controls or contradictory workload outcomes prevent acceptance. '
              'A candidate needs a fresh, balanced confirmation campaign before promotion. '
              'No adverse result is discarded or normalized using competitor variation.', '',
              'Review the committed report, JSON and trial CSV without the original execution environment. '
              'Rerunning the full historical audit with `python3 benches/analyze-local-k6.py` requires '
              'the locally retained artifact collection, which is ignored by Git and has no published restoration location. '
              'A fresh checkout contains the compact evidence, not those audit inputs. '
              'The predeclared driver, source hashes, build logs and frozen executable hashes are under '
              '`artifacts/performance-2026-10-20/local-k6-approved/`; raw k6 summaries are under '
              '`artifacts/bench/local-k6-*`. TLS private keys are excluded from reports.', '']
    (RESULTS / 'local-docker-k6.md').write_text('\n'.join(lines))
    print(json.dumps({k: evidence[k] for k in ('complete','attempts','rows','failed_attempts','measured_requests','warmup_requests')}, indent=2))


if __name__ == '__main__':
    main()
