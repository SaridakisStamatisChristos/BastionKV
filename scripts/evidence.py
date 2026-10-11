#!/usr/bin/env python3
"""Generate/check real SHA-256 of repository files (hash list itself is excluded)."""
import argparse, hashlib, sys
from pathlib import Path
root=Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--write',action='store_true');a=p.parse_args()
manifest=root/'evidence/files.sha256'
def files():
    for f in sorted(root.rglob('*')):
        r=f.relative_to(root)
        if f.is_file() and not any(x in {'target','.git','__pycache__'} for x in r.parts) and not str(r).startswith('evidence/ci/') and f!=manifest:
            yield r.as_posix(),f
current={r:hashlib.sha256(f.read_bytes()).hexdigest() for r,f in files()}
if a.write:
    manifest.write_text(''.join(f'{digest}  {name}\n' for name,digest in current.items()))
    print(f'Wrote {len(current)} real file hashes; self excluded, Git commit anchors hash list')
else:
    expected={line[66:]:line[:64] for line in manifest.read_text().splitlines()}
    bad=[name for name in sorted(current.keys()|expected.keys()) if current.get(name)!=expected.get(name)]
    if bad: print('Hash mismatch/missing/extra files:',*bad,sep='\n');sys.exit(1)
    print(f'Verified {len(current)} file hashes')
