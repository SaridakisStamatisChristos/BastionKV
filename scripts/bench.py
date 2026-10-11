#!/usr/bin/env python3
"""Capture raw benchmark samples, descriptive summaries, environment and SQLite calibration."""
import argparse, csv, hashlib, io, json, math, os, platform, sqlite3, subprocess, tempfile, time
from pathlib import Path
p=argparse.ArgumentParser()
p.add_argument('--output',default='evidence/benchmarks')
p.add_argument('--n',type=int,default=1000)
p.add_argument('--runs',type=int,default=3)
a=p.parse_args()
if not 1<=a.n<=100000 or not 1<=a.runs<=20: p.error('n 1..100000, runs 1..20')
root=Path(__file__).resolve().parents[1]
out=Path(a.output).resolve();out.mkdir(parents=True,exist_ok=True)
env=os.environ.copy();env['BASTIONKV_BENCH_N']=str(a.n)
subprocess.run(['cargo','build','--release','--bench','workloads'],cwd=root,check=True)
exe=root/'target/release/deps'
binaries=sorted(x for x in exe.glob('workloads-*') if x.is_file() and os.access(x,os.X_OK))
if len(binaries)!=1: raise RuntimeError('expected a clean single benchmark build; cargo clean before rerun if toolchains changed')
metadata={'platform':platform.platform(),'machine':platform.machine(),'python':platform.python_version(),'sqlite':sqlite3.sqlite_version,
          'rustc':subprocess.check_output(['rustc','-Vv'],text=True),'cargo':subprocess.check_output(['cargo','-V'],text=True),
          'source_files_sha256':{str(f.relative_to(root)):hashlib.sha256(f.read_bytes()).hexdigest() for f in sorted([*root.glob('src/**/*.rs'),*root.glob('benches/*.rs'),root/'Cargo.toml',root/'Cargo.lock',Path(__file__).resolve()])},
          'n':a.n,'runs':a.runs,'value_bytes':128,'key_bytes':8,'utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),
          'cpu':Path('/proc/cpuinfo').read_text() if Path('/proc/cpuinfo').exists() else 'unavailable',
          'filesystem':subprocess.check_output(['df','-T',tempfile.gettempdir()],text=True),
          'semantics':'synchronous per-transaction commit; no performance acceptance threshold; host fsync does not simulate power loss'}
(out/'environment.json').write_text(json.dumps(metadata,indent=2)+'\n')
summary=[]
for run in range(a.runs):
    raw=subprocess.check_output([str(binaries[0])],cwd=root,env=env,text=True)
    (out/f'bastion-{run}.csv').write_text(raw)
    rows=list(csv.DictReader(io.StringIO('\n'.join(x for x in raw.splitlines() if not x.startswith('#')))))
    for workload in sorted({r['workload'] for r in rows}):
        group=[r for r in rows if r['workload']==workload];ns=sorted(int(r['latency_ns']) for r in group)
        def pct(q): return ns[max(0,math.ceil(q*len(ns))-1)]
        ops=sum(int(r['logical_operations']) for r in group)
        summary.append({'run':run,'engine':'bastionkv','workload':workload,'samples':len(ns),'logical_operations':ops,'ops_per_second':ops/(sum(ns)/1e9),'median_ns':pct(.5),'p95_ns':pct(.95),'p99_ns':pct(.99)})
    metrics={r.split(',')[1]:int(r.split(',')[2]) for r in raw.splitlines() if r.startswith('#metrics')}
    metrics['storage_amplification_after_compaction']=metrics['after_compact_bytes']/metrics['live_payload_bytes']
    (out/f'metrics-{run}.json').write_text(json.dumps(metrics,indent=2)+'\n')
    # Calibration only for sequential single-key writes; identical binary keys/values, one commit each.
    with tempfile.TemporaryDirectory(prefix='bastion-sqlite-') as directory:
        con=sqlite3.connect(Path(directory)/'db.sqlite')
        assert con.execute('PRAGMA journal_mode=DELETE').fetchone()[0]=='delete'
        con.execute('PRAGMA synchronous=FULL');assert con.execute('PRAGMA synchronous').fetchone()[0]==2
        con.execute('CREATE TABLE kv(k BLOB PRIMARY KEY, v BLOB NOT NULL) WITHOUT ROWID');con.commit()
        vals=[]
        with (out/f'sqlite-{run}.csv').open('w') as f:
            w=csv.writer(f);w.writerow(['workload','sample','logical_operations','latency_ns'])
            for i in range(a.n):
                key=i.to_bytes(8,'big');value=bytes([42])*128;t=time.perf_counter_ns()
                con.execute('INSERT INTO kv VALUES(?,?)',(key,value));con.commit()
                ns=time.perf_counter_ns()-t;vals.append(ns);w.writerow(['sequential_write',i,1,ns])
        assert con.execute('SELECT COUNT(*) FROM kv').fetchone()[0]==a.n;con.close()
        vals.sort()
        summary.append({'run':run,'engine':'sqlite','workload':'sequential_write','samples':a.n,'logical_operations':a.n,'ops_per_second':a.n/(sum(vals)/1e9),'median_ns':vals[max(0,math.ceil(.5*a.n)-1)],'p95_ns':vals[max(0,math.ceil(.95*a.n)-1)],'p99_ns':vals[max(0,math.ceil(.99*a.n)-1)]})
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(f'Captured {a.runs} runs, {a.n} keys, {len(summary)} workload summaries in {out}')
