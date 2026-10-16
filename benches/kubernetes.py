#!/usr/bin/env python3
"""Run the same k6 workload in the two containers of an isolated benchmark pod."""
import argparse,csv,hashlib,json,pathlib,shlex,subprocess,time
P=argparse.ArgumentParser(description=__doc__)
P.add_argument('--context',default='homelab'); P.add_argument('--namespace',default='maximus-bench-2f2f798')
P.add_argument('--label',default='homelab'); P.add_argument('--workers',default='1,2,4')
P.add_argument('--paths',default='/plaintext,/echo'); P.add_argument('--duration',default='30s'); P.add_argument('--repeats',type=int,default=3)
P.add_argument('--generator',default='k6',choices=['k6','wrk'])
P.add_argument('--perf',action='store_true')
P.add_argument('--rate',type=int,default=0); P.add_argument('--protocol',default='plain',choices=['plain','h1','h2'])
P.add_argument('--engines',default='maximus,epoll,hyper,axum,actix'); P.add_argument('--server-cpus',default='0,2,4,6'); P.add_argument('--client-cpus',default='8,10,12,14')
A=P.parse_args(); assert A.generator!='wrk' or (A.rate==0 and A.protocol=='plain'), 'wrk campaigns use plaintext closed-loop only'
OUT=pathlib.Path('artifacts/homelab'); OUT.mkdir(parents=True,exist_ok=True)
BASE=['kubectl','--context',A.context,'-n',A.namespace,'exec','benchmark']
def command(container,*args,check=True):
 return subprocess.run(BASE+['-c',container,'--',*map(str,args)],text=True,capture_output=True,check=check)
def stop(container,name=None,signal='TERM'):
 pid_file=f'/work/{name or container}.pid'
 script='''test ! -f {file} && exit 0
pid=$(cat {file})
kill -{signal} "$pid" 2>/dev/null || true
for attempt in $(seq 1 50); do
 if ! kill -0 "$pid" 2>/dev/null || [ "$(awk '{{print $3}}' /proc/$pid/stat 2>/dev/null)" = Z ]; then rm -f {file}; exit 0; fi
 sleep 0.1
done
kill -KILL "$pid" 2>/dev/null || true
sleep 0.1
if kill -0 "$pid" 2>/dev/null && [ "$(awk '{{print $3}}' /proc/$pid/stat 2>/dev/null)" != Z ]; then exit 1; fi
rm -f {file}'''.format(file=shlex.quote(pid_file),signal=signal)
 command(container,'sh','-c',script)
def profile_control(action):
 command('server','timeout','5','sh','-c',f'echo {action} > /work/perf-control; read ack < /work/perf-ack')
def profile_start(key):
 script='rm -f /work/perf-control /work/perf-ack; mkfifo /work/perf-control /work/perf-ack; '
 script+='tids=$(ls /proc/$(cat /work/server.pid)/task | paste -sd,); '
 script+=shlex.join(['perf','stat','-D','-1','--control','fifo:/work/perf-control,/work/perf-ack','-x,','-e','cycles,instructions,cache-misses,task-clock','-o',f'/work/{key}-perf.csv'])+' -t "$tids" >/work/perf.log 2>&1 & echo $! >/work/perf.pid'
 command('server','sh','-c',script)
def snapshot(container):
 script='cat /sys/fs/cgroup/cpu.stat'
 if container=='server': script+='; cat /proc/$(cat /work/server.pid)/status; cat /proc/$(cat /work/server.pid)/stat; ps -T -p $(cat /work/server.pid) -o pid,tid,comm; head -n 17 /proc/stat; for t in /proc/$(cat /work/server.pid)/task/*; do echo TID $t; cat $t/schedstat; done' 
 values={}
 raw=command(container,'sh','-c',script).stdout
 with (OUT/f'{A.label}-snapshots.log').open('a') as f: f.write(container+' '+str(time.time())+'\n'+raw+'\n')
 for line in raw.splitlines():
  parts=line.replace(':','').split()
  if len(parts)>1 and parts[0] in ('usage_usec','VmRSS','nr_throttled','throttled_usec'): values[parts[0]]=int(parts[1])
 return values
def workload(key,path,duration,rate,log):
 env=['env',f'PATHNAME={path}',f'DURATION={duration}','VUS=128',f'RESULT=/work/{key}.json']
 if rate: env.append(f'RATE={rate}')
 if A.protocol!='plain': env+=['SSL_CERT_FILE=/work/cert.pem','URL=https://localhost:8080',f'PROTOCOL={A.protocol}']
 command('client','rm','-f',f'/work/{key}.json')
 client=['k6','run','--quiet','/work/benches/load.js'] if A.generator=='k6' else ['/work/tools/ld-linux-x86-64.so.2','--library-path','/work/tools','/work/tools/wrk','-t4','-c128','-d'+duration,'--latency','-s','/work/benches/load.lua','http://127.0.0.1:8080']
 launch=shlex.join([*env,'/work/tools/ld-linux-x86-64.so.2','--library-path','/work/tools','/work/tools/taskset','-c',A.client_cpus,*client])
 # Persist completion inside the container; a dropped exec stream must not be
 # confused with a failed client or cause a duplicate workload submission.
 output=f'/work/{key}.log'; done=f'/work/{key}.exit'
 command('client','rm','-f',output,done,done+'.tmp','/work/client.pid')
 script=f'( {launch} > {shlex.quote(output)} 2>&1 & pid=$!; echo $pid > /work/client.pid; wait $pid; status=$?; echo $status > {shlex.quote(done+".tmp")}; mv {shlex.quote(done+".tmp")} {shlex.quote(done)} ) </dev/null >/dev/null 2>&1 &'
 started=command('client','sh','-c',script,check=False)
 log.write(started.stderr); log.flush()
 seconds=float(duration[:-1])*{'s':1,'m':60,'h':3600}[duration[-1]]
 deadline=time.monotonic()+seconds+60
 while time.monotonic()<deadline:
  status=command('client','sh','-c','test -f '+shlex.quote(done)+' && cat '+shlex.quote(done),check=False)
  if status.returncode==0:
   log.write(command('client','cat',output).stdout); log.flush()
   return int(status.stdout.strip())
  time.sleep(1)
 raise RuntimeError('client completion was not recorded before its deadline')
stop('client'); stop('server','perf','INT'); stop('server')
metadata={'args':vars(A),'source_commit':subprocess.check_output(['git','rev-parse','HEAD'],text=True).strip(),'source_dirty':bool(subprocess.check_output(['git','status','--porcelain'],text=True).strip()),'date_utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'hardware':command('server','lscpu').stdout,'rustc':command('server','rustc','--version').stdout,'cgroup_cpu_max':{c:command(c,'cat','/sys/fs/cgroup/cpu.max').stdout for c in ['server','client']}}
command('server','sh','-c','cd /work && sha256sum -c SOURCE_SHA256SUMS >/work/source-verification.log')
metadata['workload_sha256']=command('server','sha256sum','/work/benches/load.js','/work/benches/load.lua').stdout
metadata['container_images']=json.loads(subprocess.check_output(['kubectl','--context',A.context,'-n',A.namespace,'get','pod','benchmark','-o','json'],text=True))['status']['containerStatuses']
metadata.update(harness_sha256=hashlib.sha256(pathlib.Path(__file__).read_bytes()).hexdigest(),source_manifest_sha256=command('server','sha256sum','/work/SOURCE_SHA256SUMS').stdout,binary_sha256=command('server','sha256sum','/work/benches/baselines/target/release/maximus-comparison').stdout,cpu_topology=command('server','lscpu','-e=CPU,CORE,SOCKET,ONLINE').stdout,effective_cpus={c:command(c,'cat','/sys/fs/cgroup/cpuset.cpus.effective').stdout for c in ['server','client']})
(OUT/f'{A.label}-environment.json').write_text(json.dumps(metadata,indent=2))
rows=[]
try:
 for repeat in range(A.repeats):
  engines=A.engines.split(','); engines=engines[repeat%len(engines):]+engines[:repeat%len(engines)]
  for workers in map(int,A.workers.split(',')):
   cpus=','.join(A.server_cpus.split(',')[:workers]); assert len(cpus.split(','))==workers
   for path in A.paths.split(','):
    for engine in engines:
     key=f'{A.label}-{engine}-{path.strip("/").replace("/","-")}-w{workers}-n{repeat}'
     env=[f'WORKERS={workers}']
     if A.protocol!='plain': env+=['MAXIMUS_CERT=/work/cert.pem','MAXIMUS_KEY=/work/key.pem',f'PROTOCOL={A.protocol}']
     launch=shlex.join(['nohup','env',*env,'taskset','-c',cpus,'/work/benches/baselines/target/release/maximus-comparison',engine])
     command('server','sh','-c',launch+' </dev/null >/work/server.log 2>&1 & echo $! >/work/server.pid')
     try:
      for attempt in range(100):
       if 'ready' in command('server','cat','/work/server.log').stdout: break
       time.sleep(.1)
      else: raise RuntimeError(command('server','cat','/work/server.log').stdout)
      with (OUT/f'{key}.log').open('w') as log:
       assert workload('warmup',path,'3s',0,log)==0,'warmup failed'
       if A.perf: profile_start(key)
       before={c:snapshot(c) for c in ['server','client']}; started=time.monotonic()
       if A.perf: profile_control('enable')
       code=workload(key,path,A.duration,A.rate,log)
       if A.perf: profile_control('disable')
       after={c:snapshot(c) for c in ['server','client']}; wall=time.monotonic()-started
       if A.perf: stop('server','perf','INT')
      raw=command('client','cat',f'/work/{key}.json').stdout; (OUT/f'{key}.json').write_text(raw)
      summary=json.loads(raw); metrics=summary['metrics']; metric=lambda n,k,d=0:metrics.get(n,{}).get('values',{}).get(k,d)
      count=metric('http_reqs','count'); cpu={c:after[c]['usage_usec']-before[c]['usage_usec'] for c in before}
      measured_seconds=summary['state']['testRunDurationMs']/1000
      assert metric('checks','passes')+metric('checks','fails')==count, 'not every response was validated'
      row=dict(generator=A.generator,engine=engine,path=path,workers=workers,protocol=A.protocol,offered_rps=A.rate,repeat=repeat,requests=count,rps=metric('http_reqs','rate'),p50_ms=metric('http_req_duration','med'),p99_ms=metric('http_req_duration','p(99)'),p999_ms=metric('http_req_duration','p(99.9)'),failures=metric('http_req_failed','passes'),checks_failed=metric('checks','fails'),dropped=metric('dropped_iterations','count'),network_errors=metric('network_errors','count'),cpu_us_per_request=cpu['server']/max(count,1),server_cpu_percent=cpu['server']/measured_seconds/10000,client_cpu_percent=cpu['client']/measured_seconds/10000,measurement_wall_seconds=wall,rss_kib=after['server']['VmRSS'],observed_rss_kib=max(before['server']['VmRSS'],after['server']['VmRSS']),server_throttled_usec=after['server']['throttled_usec']-before['server']['throttled_usec'],client_throttled_usec=after['client']['throttled_usec']-before['client']['throttled_usec'],exit=code)
      if A.perf:
       raw_perf=command('server','cat',f'/work/{key}-perf.csv').stdout; (OUT/f'{key}-perf.csv').write_text(raw_perf)
       events={r[2]:float(r[0]) for r in csv.reader(raw_perf.splitlines()) if len(r)>2 and r[2] in ('cycles','instructions','cache-misses','task-clock')}
       assert set(events)=={'cycles','instructions','cache-misses','task-clock'} and events['cycles']>0,raw_perf
       row.update(cycles_per_request=events['cycles']/count,instructions_per_request=events['instructions']/count,cache_misses_per_request=events['cache-misses']/count,profile_task_clock_ms=events['task-clock'])
      rows.append(row)
      with (OUT/f'{A.label}.csv').open('w') as file:
       writer=csv.DictWriter(file,fieldnames=list(row)); writer.writeheader(); writer.writerows(rows)
      print(json.dumps(row),flush=True)
     finally:
      try: stop('client')
      finally:
       try: stop('server','perf','INT')
       finally: stop('server')
finally:
 try: stop('client')
 finally:
  try: stop('server','perf','INT')
  finally: stop('server')
