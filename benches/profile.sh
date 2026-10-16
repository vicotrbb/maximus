#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
mkdir -p artifacts/profile artifacts/tools/flamegraph
revision=41fee1f99f9276008b7cd112fca19dc3ea84ac32
for tool in stackcollapse-perf.pl flamegraph.pl; do
    curl -fsSL "https://raw.githubusercontent.com/brendangregg/FlameGraph/$revision/$tool" -o "artifacts/tools/flamegraph/$tool"
done
docker run --rm -d --name maximus-profile --network host --cpuset-cpus 0 --cap-add PERFMON --cap-add SYS_PTRACE --security-opt seccomp=unconfined -v maximus-bench-target:/target:ro -v "$PWD/artifacts/profile:/profile" maximus-tools /target/release/maximus-comparison maximus
trap 'docker rm -f maximus-profile >/dev/null 2>&1 || true' EXIT
docker exec maximus-profile perf record -F 99 -e cpu-clock -g --call-graph dwarf -o /profile/perf.data -p 1 -- sleep 15 > artifacts/profile/record.log 2>&1 &
record_pid=$!
docker run --rm --network host --cpuset-cpus 2 -u 0 -v "$PWD/benches/load.js:/load.js:ro" -v "$PWD/artifacts/profile:/results" -e DURATION=15s grafana/k6:1.6.0 run --quiet /load.js > artifacts/profile/k6.log 2>&1
wait "$record_pid"
docker exec maximus-profile perf script -i /profile/perf.data > artifacts/profile/perf.script
perl artifacts/tools/flamegraph/stackcollapse-perf.pl artifacts/profile/perf.script > artifacts/profile/stacks.folded
perl artifacts/tools/flamegraph/flamegraph.pl --title 'Maximus HTTP/1 CPU-clock samples' artifacts/profile/stacks.folded > artifacts/profile/flamegraph.svg
