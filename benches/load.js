import http from 'k6/http';
import { check } from 'k6';
import { Counter } from 'k6/metrics';
const iterationStarts = new Counter('iteration_starts');
const path = __ENV.PATHNAME || '/plaintext';
const body = 'x'.repeat(Number(__ENV.BODY_BYTES || 1024));
const expected = path === '/compress' ? 'Hello, compression!'.repeat(100) : path === '/echo' ? body : path === '/sse' ? 'data:one\ndata:two\ndata:three\n\n' : path === '/error' ? 'handler error' : path === '/json' ? '{"message":"Hello, World!"}' : path.startsWith('/users/') ? '42' : 'Hello, World!';
const expectedStatus = path === '/error' ? 500 : 200;
http.setResponseCallback(http.expectedStatuses(expectedStatus));
const scenario = __ENV.RATE ? {
  executor: 'constant-arrival-rate', rate: Number(__ENV.RATE), timeUnit: '1s',
  duration: __ENV.DURATION || '30s', preAllocatedVUs: Number(__ENV.VUS || 128),
} : { executor: 'constant-vus', vus: Number(__ENV.VUS || 64), duration: __ENV.DURATION || '30s' };
export const options = {
  scenarios: { measured: scenario },
  tlsVersion: { min: 'tls1.3', max: 'tls1.3' },
  summaryTrendStats: ['avg', 'med', 'p(99)', 'p(99.9)', 'max'],
  thresholds: { http_req_failed: ['rate<0.001'], checks: ['rate==1'], ...(__ENV.RATE ? { dropped_iterations: ['count==0'] } : {}) },
};
export default function () {
  iterationStarts.add(1);
  const url = (__ENV.URL || 'http://127.0.0.1:8080') + path;
  const r = path === '/echo' ? http.post(url, body) : http.get(url, path === '/compress' ? { headers: { 'Accept-Encoding': 'br;q=1, gzip;q=0.8, identity;q=0.5' } } : {});
  check(r, { 'correct status, body and protocol': r => r.status === expectedStatus && (path !== '/compress' || r.headers['Content-Encoding'] === 'gzip') && (path === '/sse' ? r.body.replace(/^data: /gm, 'data:') : r.body) === expected && r.proto === (__ENV.PROTOCOL === 'h2' ? 'HTTP/2.0' : 'HTTP/1.1') });
}
export function handleSummary(data) {
  return { [__ENV.RESULT || '/results/summary.json']: JSON.stringify(data, null, 2) };
}
