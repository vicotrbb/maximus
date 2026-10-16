-- Response validation stays enabled; this is not an unchecked throughput run.
local threads = {}
failures, checked = 0, 0
local path = os.getenv("PATHNAME") or "/plaintext"
local expected = "Hello, World!"
if path == "/echo" then
  wrk.method, wrk.body = "POST", string.rep("x", 1024)
  expected = wrk.body
elseif path == "/json" then
  expected = '{"message":"Hello, World!"}'
elseif string.sub(path, 1, 7) == "/users/" then
  expected = "42"
end
wrk.path = path
function setup(thread) table.insert(threads, thread) end
function response(status, headers, body)
  checked = checked + 1
  if status ~= 200 or body ~= expected then failures = failures + 1 end
end
function done(summary, latency, requests)
  local failed, count = 0, 0
  for _, thread in ipairs(threads) do
    failed, count = failed + thread:get("failures"), count + thread:get("checked")
  end
  local errors = summary.errors
  local output = assert(io.open(os.getenv("RESULT"), "w"))
  output:write(string.format([[{"state":{"testRunDurationMs":%.3f},"metrics":{
"http_reqs":{"values":{"count":%d,"rate":%.6f}},
"http_req_duration":{"values":{"med":%.6f,"p(99)":%.6f,"p(99.9)":%.6f}},
"http_req_failed":{"values":{"passes":%d}},
"network_errors":{"values":{"count":%d}},
"checks":{"values":{"passes":%d,"fails":%d}}}}
]], summary.duration / 1000, summary.requests, summary.requests * 1000000 / summary.duration,
  latency:percentile(50) / 1000, latency:percentile(99) / 1000, latency:percentile(99.9) / 1000,
  errors.status, errors.connect + errors.read + errors.write + errors.timeout, count - failed, failed))
  output:close()
  if failed > 0 or errors.status > 0 or errors.connect + errors.read + errors.write + errors.timeout > 0 or count ~= summary.requests or count == 0 then os.exit(1) end
end
