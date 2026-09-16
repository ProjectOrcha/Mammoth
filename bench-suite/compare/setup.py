#!/usr/bin/env python3
"""Fetch pinned official benchmark runtimes into an ignored project directory."""
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import re
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
DEST = ROOT / 'target/comparison-tools'
PACKAGES = {
 'jdk': ('https://github.com/adoptium/temurin17-binaries/releases/download/jdk-17.0.20.1%2B1/OpenJDK17U-jdk_aarch64_mac_hotspot_17.0.20.1_1.tar.gz', 'sha256', '.sha256.txt'),
 'hadoop': ('https://downloads.apache.org/hadoop/common/hadoop-3.5.0/hadoop-3.5.0.tar.gz', 'sha512', '.sha512'),
 'spark': ('https://downloads.apache.org/spark/spark-4.2.0/spark-4.2.0-bin-hadoop3.tgz', 'sha512', '.sha512'),
}

def fetch(item):
 name, (url, algorithm, suffix) = item
 archive = DEST / url.rsplit('/', 1)[1]
 with urllib.request.urlopen(url+suffix, timeout=30) as response:
  text = response.read().decode()
 expected = re.search(r'\b[0-9a-fA-F]{'+str(hashlib.new(algorithm).digest_size*2)+r'}\b', text).group().lower()
 if not archive.exists():
  partial = archive.with_suffix('.partial')
  print(f'Downloading {name}...', flush=True)
  with urllib.request.urlopen(url, timeout=60) as response, partial.open('wb') as output:
   while chunk := response.read(1024*1024): output.write(chunk)
  partial.rename(archive)
 with archive.open('rb') as source:
  actual = hashlib.file_digest(source, algorithm).hexdigest()
 if actual != expected: raise RuntimeError(f'{name} checksum mismatch')
 target = DEST / name
 if not target.exists():
  target.mkdir()
  with tarfile.open(archive) as bundle: bundle.extractall(target, filter='data')
 print(f'{name}: verified and extracted', flush=True)
 return name, {'url':url, algorithm:actual, 'directory':str(target.relative_to(ROOT))}

if __name__ == '__main__':
 DEST.mkdir(parents=True, exist_ok=True)
 with ThreadPoolExecutor(max_workers=3) as pool: result=dict(pool.map(fetch, PACKAGES.items()))
 (DEST/'downloads.json').write_text(json.dumps(result,indent=2)+'\n')
