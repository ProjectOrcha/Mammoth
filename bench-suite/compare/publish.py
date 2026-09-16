#!/usr/bin/env python3
"""Validate raw same-Mac evidence, then render the shared UI/website/docs snapshot."""
import argparse
import json
import math
from pathlib import Path
import statistics

ROOT = Path(__file__).resolve().parents[2]
RESULTS = ROOT / 'bench-suite/results'
PHASES = [('write','Write','MiB/s'),('read','Read · empty Mammoth cache','MiB/s'),
          ('read_cached','Repeated read*','MiB/s'),('sort','Line sort','MiB/s'),
          ('wordcount','Word count','MiB/s'),('create','Create empty file','ops/s'),
          ('stat','Stat','ops/s'),('rename','Rename','ops/s'),('delete','Delete','ops/s')]
CAVEATS = [
 'The source manifest identifies the measured build. Later code changes require new measurements; this snapshot does not certify the current branch head.',
 'Single physical Mac. These are small same-host workloads, not a physical distributed cluster or an overall winner.',
 'Mammoth uses direct local backend calls and six worker directories. HDFS uses a Java RPC client, one NameNode and six DataNode JVMs over loopback. Spark runs local[18] on HDFS, with four partitions per job and four concurrent per-file jobs; it is the compute comparison, not a storage alternative.',
 'Eight 8 MiB files, four clients, 4 MiB blocks, seed 42, one and three replicas; 200 empty files per metadata phase. Median of three measured iterations after one excluded warmup. Engine groups run sequentially; service startup, setup, compute validation, layout checks and cleanup are excluded. Read byte-count and checksum verification are timed. All measured runs are retained.',
 'OS caches stay warm. The first read clears only Mammoth’s application cache. Repeated read* reuses Mammoth’s verified memory cache; HDFS performs a second normal read. Neither measures cold storage bandwidth. Logical MiB/s can exceed SSD bandwidth.',
 'Memory policies differ: Mammoth has a 256 MiB read cache and a 32 MiB accounting target per job; Spark has a 1 GiB driver heap, and each HDFS daemon a 512 MiB heap. These are not equivalent RSS limits. CPU, peak RSS and network saturation were not measured.',
 'Compute uses 335,544 fixed 25-byte records per file, 4,096 numeric keys and three repeated words. Outputs have identical logical records, but Mammoth writes one file per job and Spark writes four part files plus commit markers. This is not TeraSort, HiBench or Hadoop MapReduce.',
 'Writes wait for requested replicas. Mammoth syncs replica files and SQLite WAL metadata. HDFS uses hsync and DataNode sync-on-close; Spark uses an explicit replicated, synced text output writer. Replica placement and all output records/checksums are verified. These checks do not constitute a crash-durability test.',
 'No native Hadoop library is available in this Mac distribution, so Hadoop uses Java fallbacks. Do not extrapolate these results to tuned Linux clusters. Empty metadata files do not exercise replication.'
]

def dump(path, value):
 path.parent.mkdir(parents=True, exist_ok=True)
 path.write_text(json.dumps(value, indent=2, ensure_ascii=False)+'\n')

def clean(value):
 if isinstance(value, dict):return {k:clean(v) for k,v in value.items()}
 if isinstance(value, list):return [clean(v) for v in value]
 if isinstance(value, str):return value.replace(str(ROOT),'<workspace>')
 return value

def require(condition, message):
 if not condition:raise ValueError(message)

def validate(report, phases):
 require(report['verified'] is True and report['cleanup_complete'] is True, 'verification and cleanup must both succeed')
 expected={(p,r,i) for p in phases for r in [1,3] for i in [1,2,3]}
 require(len(report['samples'])==len(expected), 'incomplete or extra samples')
 require({(s['phase'],s['replication'],s['iteration']) for s in report['samples']}==expected, 'missing or duplicate iterations')
 opts=report['options']
 for key,value in dict(file_size=8388608,files=8,concurrency=4,iterations=3,warmups=1,block_size=4194304,seed=42,replications=[1,3]).items():
  require(opts[key]==value, f'workload mismatch: {key}')
 for sample in report['samples']:
  wall=sample['wall_seconds']
  require(math.isfinite(wall) and wall>0, 'invalid phase duration')
  require(math.isfinite(sample['latency_p99_ms']) and sample['latency_p99_ms']>=0, 'invalid p99 latency')
  phase=sample['phase']; compute=phase in ['sort','wordcount']; metadata=phase in ['create','stat','rename','delete']
  require(sample['operations']==(200 if metadata else 8), 'incorrect operation count')
  require(sample['bytes']==(0 if metadata else (67108800 if compute else 67108864)), 'incorrect byte count')
  if metadata:require(math.isclose(sample['ops_per_second'],200/wall,rel_tol=1e-9), 'incorrect operation rate')
  else:require(math.isclose(sample['aggregate_mib_s'],sample['bytes']/1048576/wall,rel_tol=1e-9), 'incorrect throughput')

def aggregate(report, phase, replicas, unit):
 samples=[s for s in report['samples'] if s['phase']==phase and s['replication']==replicas]
 if not samples:return None
 rates=[s['aggregate_mib_s' if unit=='MiB/s' else 'ops_per_second'] for s in samples]
 return dict(median=statistics.median(rates),min=min(rates),max=max(rates),median_p99_ms=statistics.median(s['latency_p99_ms'] for s in samples))

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--from-run',type=Path)
 args=parser.parse_args()
 snapshot=json.loads((ROOT/'web/public/benchmarks/comparison.json').read_text()) if not args.from_run else None
 reports={}
 for name in ['mammoth','hdfs','spark','environment']:
  source=args.from_run/(name+'.json') if args.from_run else RESULTS/snapshot['artifacts'][name]
  reports[name]=clean(json.loads(source.read_text()))
 validate(reports['mammoth'],[p[0] for p in PHASES])
 validate(reports['hdfs'],['write','read','read_repeated','create','stat','rename','delete'])
 validate(reports['spark'],['sort','wordcount'])
 require(reports['mammoth']['environment']['engine']=='local-memory-parallel-v3', 'unexpected Mammoth engine')
 require(reports['mammoth']['environment']['profile']=='release', 'Mammoth was not a release build')
 require(reports['environment']['verified'] is True and reports['environment']['test_services_stopped'] is True, 'environment checks incomplete')
 environment=reports['environment']
 date=environment['measured_at_utc'][:10]
 prefix=date+'-mac'
 os_version=next(line.split(':',1)[1].strip() for line in environment['os'].splitlines() if line.startswith('ProductVersion:'))
 machine=f"{environment['cpu']} · {environment['logical_cpus']} logical cores · {environment['memory_bytes']/1073741824:g} GiB · macOS {os_version}"
 rows=[]
 for replicas in [1,3]:
  for phase,label,unit in PHASES:
   rows.append(dict(phase=phase,label=label,replication=replicas,unit=unit,
    mammoth=aggregate(reports['mammoth'],phase,replicas,unit),
    hdfs=aggregate(reports['hdfs'],'read_repeated' if phase=='read_cached' else phase,replicas,unit),
    spark=aggregate(reports['spark'],phase,replicas,unit)))
 comparison=dict(schema_version=1,date=date,verified=True,scope='same-Mac local workload comparison',
  machine=machine,
  versions=dict(mammoth='local-memory-parallel-v3',hdfs=reports['hdfs']['version'],spark=reports['spark']['version']),
  rows=rows,caveats=CAVEATS,artifacts={n:f'{prefix}-{n}.json' for n in reports})
 for name,report in reports.items():
  dump(RESULTS/f'{prefix}-{name}.json',report)
  dump(ROOT/'web/public/benchmarks'/f'{prefix}-{name}.json',report)
 dump(RESULTS/f'{prefix}-comparison.json',comparison)
 dump(ROOT/'web/public/benchmarks/comparison.json',comparison)
 dump(ROOT/'web/public/benchmarks/latest.json',reports['mammoth'])
 dump(ROOT/'web/public/benchmarks/environment.json',reports['environment'])
 dump(ROOT/'ui/src/lib/data/benchmark-comparison.json',comparison)
 dump(ROOT/'ui/static/benchmarks/comparison.json',comparison)
 for name,report in reports.items():dump(ROOT/'ui/static/benchmarks'/f'{prefix}-{name}.json',report)
 doc=f'# Measured Mac comparison\n\nMeasured {date}. All three engines completed with verified results and cleaned benchmark namespaces.\n\n'
 doc+='Mammoth `local-memory-parallel-v3`; Hadoop HDFS '+comparison['versions']['hdfs']+'; Spark '+comparison['versions']['spark']+' on HDFS. '+comparison['machine']+'.\n\n'
 for replicas in [1,3]:
  doc+=f'## {replicas} replica'+('s' if replicas>1 else '')+'\n\n| Operation | Unit | Mammoth | Hadoop HDFS | Spark on HDFS |\n| --- | --- | ---: | ---: | ---: |\n'
  for row in rows:
   if row['replication']!=replicas:continue
   values=['—' if row[n] is None else f"{row[n]['median']:,.2f}" for n in ['mammoth','hdfs','spark']]
   doc+='| '+row['label']+' | '+row['unit']+' | '+' | '.join(values)+' |\n'
  doc+='\n'
 doc+='Values are medians; higher rates mean more work per second. A dash means that engine was not measured for that operation. Ranges and per-iteration p99 latency are retained in the downloadable comparison JSON.\n\n## Interpretation and limits\n\n'+'\n\n'.join(CAVEATS)+'\n\n## Reproduce and inspect\n\n```bash\ncargo build --release --locked -p mammoth-cli\npython3 bench-suite/compare/setup.py\npython3 bench-suite/compare/run.py --output target/mac-comparison-new\npython3 bench-suite/compare/publish.py --from-run target/mac-comparison-new\n```\n\nPinned Mac ARM64 runtimes are checksum-verified and stored under ignored `target/comparison-tools`. The runner uses a new disposable directory, binds test services to loopback, and stops them on success or failure. Namespace cleanup excludes the retained logs and daemon storage directories. Do not run other benchmarks or builds concurrently. The source manifest records the measured binary, base revision and dirty source hashes; it identifies the measurement, not a pristine Git commit.\n\n'
 for name in [*reports,'comparison']:doc+=f'- [{name.title()} raw evidence](../bench-suite/results/{prefix}-{name}.json)\n'
 doc+='\n[Harness source and methodology](../bench-suite/compare/README.md) · [Historical JSON engine report](BENCHMARKS-LEGACY.md) · [Linux collector](../bench-suite/run_linux.py). No current Linux or multi-machine competitive result has been collected.\n'
 (ROOT/'docs/BENCHMARKS.md').write_text(doc)
 print(doc)
if __name__=='__main__':main()
