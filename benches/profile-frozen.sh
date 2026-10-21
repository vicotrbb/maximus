#!/bin/sh
# Software CPU-clock sampling of a response-checked frozen binary in a private network.
set -eu
cd "$(dirname "$0")/.."
binary=${1:?usage: profile-frozen.sh /target/frozen/binary /path output-directory}
path=${2:-/plaintext}
out=${3:-artifacts/profile-frozen}
mkdir -p "$out"
out=$(cd "$out" && pwd)
name=maximus-profile-$$
docker run --rm -d --name "$name" --network none --cpuset-cpus 0 \
 --cap-add PERFMON --cap-add SYS_PTRACE --security-opt seccomp=unconfined \
 -v maximus-bench-target:/target:ro -v "$out:/profile" maximus-tools \
 "$binary" maximus > "$out/container-id.txt"
trap 'docker rm -f "$name" >/dev/null 2>&1 || true' EXIT
# Record PMU availability independently; unsupported counters are diagnostic data.
docker exec "$name" perf stat -e cycles,instructions -p 1 -- sleep 1 > "$out/pmu.log" 2>&1 || true
docker exec "$name" sha256sum "$binary" > "$out/binary.sha256"
docker exec "$name" perf record -F 199 -e cpu-clock -g --call-graph dwarf \
 -o /profile/perf.data -p 1 -- sleep 20 > "$out/record.log" 2>&1 &
record_pid=$!
docker run --rm --network "container:$name" --cpuset-cpus 1,2 -u 0 \
 -v "$PWD/benches/load.js:/load.js:ro" -v "$out:/results" \
 -e DURATION=20s -e "PATHNAME=$path" -e RESULT=/results/summary.json \
 grafana/k6:1.6.0 run --quiet /load.js > "$out/k6.log" 2>&1
wait "$record_pid"
docker exec "$name" perf script -i /profile/perf.data > "$out/perf.script"
perl artifacts/tools/flamegraph/stackcollapse-perf.pl "$out/perf.script" > "$out/stacks.folded"
perl artifacts/tools/flamegraph/flamegraph.pl --title 'Maximus software CPU-clock samples' "$out/stacks.folded" > "$out/flamegraph.svg"
