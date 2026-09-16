#!/usr/bin/env python3
"""Sequential same-Mac measurements; no JVM runtimes or services installed globally."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import shutil
import socket
import subprocess
import time
import urllib.request
from xml.sax.saxutils import escape

ROOT=Path(__file__).resolve().parents[2]
TOOLS=ROOT/'target/comparison-tools'

def free_port():
 with socket.socket() as s: s.bind(('127.0.0.1',0));return s.getsockname()[1]
def capture(cmd):
 r=subprocess.run(cmd,cwd=ROOT,text=True,capture_output=True,timeout=30)
 return r.stdout.strip() or r.stderr.strip()
def xml(path,items):
 path.write_text('<?xml version="1.0"?>\n<configuration>\n'+''.join(f'<property><name>{escape(k)}</name><value>{escape(str(v))}</value></property>\n' for k,v in items.items())+'</configuration>\n')
def sha(path):
 with path.open('rb') as f: return hashlib.file_digest(f,'sha256').hexdigest()

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True)
 args=parser.parse_args()
 if platform.system()!='Darwin' or platform.machine()!='arm64':parser.error('This pinned setup is for macOS ARM64.')
 output=args.output.resolve();output.mkdir(parents=True,exist_ok=False)
 java=next((TOOLS/'jdk').glob('*/Contents/Home'));hadoop=next((TOOLS/'hadoop').glob('hadoop-*'));spark=next((TOOLS/'spark').glob('spark-*'))
 env={**os.environ,'JAVA_HOME':str(java),'HADOOP_HOME':str(hadoop),'HADOOP_LOG_DIR':str(output/'logs'),'HADOOP_HEAPSIZE_MAX':'512m','SPARK_LOCAL_IP':'127.0.0.1'}
 (output/'logs').mkdir();processes=[]
 def run(cmd,label,extra=None):
  print(label,flush=True)
  with (output/(label+'.log')).open('w') as log:
   subprocess.run(list(map(str,cmd)),cwd=ROOT,env={**env,**(extra or {})},stdout=log,stderr=subprocess.STDOUT,check=True)
 def start(cmd,label,extra=None):
  pid_dir=output/('pids-'+label);pid_dir.mkdir()
  log=(output/(label+'.log')).open('w')
  p=subprocess.Popen(list(map(str,cmd)),cwd=ROOT,env={**env,'HADOOP_PID_DIR':str(pid_dir),**(extra or {})},stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
  log.close();processes.append(p);return p
 paths=[ROOT/'Cargo.toml',ROOT/'Cargo.lock']+sorted((ROOT/'crates').rglob('*.rs'))+sorted((ROOT/'crates').rglob('Cargo.toml'))+sorted((ROOT/'bench-suite/compare').glob('*.*'))
 manifest={'measured_at_utc':datetime.now(timezone.utc).isoformat(),'cpu':capture(['sysctl','-n','machdep.cpu.brand_string']),
  'logical_cpus':int(capture(['sysctl','-n','hw.logicalcpu'])),'memory_bytes':int(capture(['sysctl','-n','hw.memsize'])),
  'os':capture(['sw_vers']),'architecture':platform.machine(),'rustc':capture(['rustc','-V']),
  'java':capture([str(java/'bin/java'),'-version']),'base_revision':capture(['git','rev-parse','HEAD']),
  'source_sha256':{str(p.relative_to(ROOT)):sha(p) for p in paths},'binary_sha256':sha(ROOT/'target/release/mammoth'),
  'downloads':json.loads((TOOLS/'downloads.json').read_text()),'cache_policy':'OS caches retained, no cache-dropping privileges used',
  'timing':'sequential engine groups; one warmup and three measurements; service/JVM startup excluded',
  'limits':'single Mac; small finite-key datasets; different execution paths and memory policies; not a distributed or resource-normalized contest'}
 (output/'environment.json').write_text(json.dumps(manifest,indent=2)+'\n')
 try:
  # Run Mammoth while baseline JVMs are stopped to avoid their memory/CPU overhead.
  run([ROOT/'target/release/mammoth','--local-root',output/'mammoth-store','bench','suite','--size','8MiB','--files','8','--concurrency','4','--ops','200','--iterations','3','--warmups','1','--replication','1,3','--block-size','4MiB','--read-cache','256MiB','--compute-memory','32MiB','--seed','42','--report',output/'mammoth.json'],'mammoth')
  rpc,http=free_port(),free_port(); conf=output/'conf';conf.mkdir()
  core={'fs.defaultFS':f'hdfs://127.0.0.1:{rpc}','hadoop.tmp.dir':output/'hadoop-tmp','io.file.buffer.size':65536}
  common={'dfs.namenode.name.dir':output/'namenode','dfs.namenode.rpc-address':f'127.0.0.1:{rpc}',
   'dfs.namenode.http-address':f'127.0.0.1:{http}','dfs.replication':3,'dfs.blocksize':4194304,
   'dfs.bytes-per-checksum':4096,'dfs.checksum.type':'CRC32C','dfs.datanode.synconclose':'true',
   'dfs.permissions.enabled':'false','dfs.namenode.safemode.threshold-pct':0,'dfs.namenode.safemode.extension':0,
   'dfs.namenode.datanode.registration.ip-hostname-check':'false','dfs.datanode.address':'127.0.0.1:0',
   'dfs.datanode.ipc.address':'127.0.0.1:0','dfs.datanode.http.address':'127.0.0.1:0','dfs.datanode.hostname':'127.0.0.1'}
  xml(conf/'core-site.xml',core);xml(conf/'hdfs-site.xml',common)
  shutil.copy(hadoop/'etc/hadoop/log4j.properties',conf/'log4j.properties')
  env['HADOOP_CONF_DIR']=str(conf)
  run([hadoop/'bin/hdfs','namenode','-format','-nonInteractive'],'format')
  start([hadoop/'bin/hdfs','namenode'],'namenode')
  for i in range(6):
   dn=output/f'conf-dn{i}';dn.mkdir();xml(dn/'core-site.xml',core);xml(dn/'hdfs-site.xml',{**common,'dfs.datanode.data.dir':output/f'datanode-{i}'})
   shutil.copy(conf/'log4j.properties',dn/'log4j.properties')
   start([hadoop/'bin/hdfs','--config',dn,'datanode'],f'datanode-{i}')
  deadline=time.monotonic()+120
  while time.monotonic()<deadline:
   if any(p.poll() is not None for p in processes):raise RuntimeError('HDFS daemon exited; inspect logs')
   try:
    with urllib.request.urlopen(f'http://127.0.0.1:{http}/jmx?qry=Hadoop:service=NameNode,name=NameNodeInfo',timeout=2) as response:info=json.load(response)
    if len(json.loads(info['beans'][0]['LiveNodes']))==6:break
   except (OSError,KeyError,IndexError):pass
   time.sleep(.25)
  else:raise RuntimeError('Six DataNodes did not register')
  classes=output/'classes';classes.mkdir()
  classpath=subprocess.check_output([str(hadoop/'bin/hadoop'),'classpath','--glob'],env=env,text=True).strip()
  run([java/'bin/javac','-cp',classpath,'-d',classes,ROOT/'bench-suite/compare/StorageBaseline.java'],'compile-hdfs')
  run([java/'bin/java','-Xmx1g','-cp',str(classes)+os.pathsep+classpath,'StorageBaseline',conf,output/'hdfs.json'],'hdfs')
  run([java/'bin/javac','-cp',str(spark/'jars/*'),'-d',classes,ROOT/'bench-suite/compare/SparkBaseline.java'],'compile-spark')
  run([java/'bin/jar','cf',output/'comparison.jar','-C',classes,'.'],'jar')
  run([spark/'bin/spark-submit','--driver-memory','1g','--class','SparkBaseline',output/'comparison.jar',conf,output/'spark.json',output/'spark-scratch'],'spark')
  for name in ['mammoth','hdfs','spark']:
   report=json.loads((output/(name+'.json')).read_text());assert report['verified'] and report['cleanup_complete']
  manifest['completed_at_utc']=datetime.now(timezone.utc).isoformat();manifest['verified']=True
  print('All engine outputs verified. Reports: '+str(output),flush=True)
 finally:
  for p in processes:
   if p.poll() is None:os.killpg(p.pid,signal.SIGTERM)
  for p in processes:
   try:p.wait(timeout=10)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait()
  manifest['test_services_stopped']=True
  (output/'environment.json').write_text(json.dumps(manifest,indent=2)+'\n')
if __name__=='__main__':main()
