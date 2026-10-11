#!/usr/bin/env python3
"""Real Linux short-write/ENOSPC-like failure via RLIMIT_FSIZE; no simulated success."""
import os, resource, signal, subprocess, tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
binary=root/'target/debug/bastionkv'
with tempfile.TemporaryDirectory(prefix='bastion-os-fault-') as tmp:
    db=Path(tmp)/'db'
    def run(*args,**kw): return subprocess.run([str(binary),*map(str,args)],capture_output=True,**kw)
    assert run('init',db).returncode==0
    assert run('put',db,'base','safe').returncode==0
    wal=next(db.glob('*.bkw'));base=wal.stat().st_size
    def limit():
        signal.signal(signal.SIGXFSZ,signal.SIG_IGN)
        resource.setrlimit(resource.RLIMIT_FSIZE,(base+30,base+30))
    failed=run('put',db,'new','x'*1000,preexec_fn=limit)
    assert failed.returncode!=0 and b'File too large' in failed.stderr,failed.stderr
    assert wal.stat().st_size==base+30,'expected real partial write at limit'
    assert run('verify',db).returncode==0
    assert run('get',db,'base').stdout.strip()==b'73616665'
    assert run('get',db,'new').returncode!=0
    assert wal.stat().st_size==base,'recovery must discard the torn frame'
    # Non-root permissions fixture: empty database path loses permissions before open.
    os.chmod(db,0)
    try:
        denied=run('stats',db)
        if os.geteuid()!=0: assert denied.returncode!=0
        else: print('permission denial UNVERIFIED under root; CI non-root exercises it')
    finally: os.chmod(db,0o700)
    # Offline verification must need only read permission, including its shared LOCK.
    for path in db.iterdir(): os.chmod(path,0o400)
    os.chmod(db,0o500)
    try:
        readonly=run('verify',db)
        assert readonly.returncode==0,readonly.stderr
        if os.geteuid()!=0:
            assert run('stats',db).returncode!=0,'writable open unexpectedly allowed'
            print('Read-only offline verify and permission-denied writer: PASS')
    finally:
        os.chmod(db,0o700)
        for path in db.iterdir(): os.chmod(path,0o600)
print('Real short write, failed append, atomic prefix recovery, tail truncation: PASS')
