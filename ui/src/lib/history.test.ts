import { expect, it } from 'vitest';
import { clusterReport } from './demo';
import { recordSnapshot, replicaFlow, snapshotHeat, snapshotTopology, type Snapshot } from './history';

it('records only local snapshots, samples every ten seconds and bounds memory', () => {
  const report = clusterReport();
  expect(recordSnapshot([], report, 1)).toEqual([]);
  report.capabilities = { local: true, distributed_metrics: false, history: false, jobs: true };
  let history: Snapshot[] = recordSnapshot([], report, 10000);
  expect(recordSnapshot(history, report, 19999)).toBe(history);
  for (let i = 2; i <= 200; i++) history = recordSnapshot(history, report, i * 10000);
  expect(history).toHaveLength(181);
  expect(history[0].at).toBe(200000);
  expect(history[180].at).toBe(2000000);
});

it('replays usage and placement without inventing network activity', () => {
  const report = clusterReport();
  const heat = snapshotHeat(report);
  expect(heat[0].usage).toBe(report.nodes[0].used / report.nodes[0].capacity * 100);
  expect(Number.isNaN(heat[0].read_qps)).toBe(true);
  expect(snapshotTopology(report).links).toEqual([]);
  const flow = replicaFlow(report);
  expect(flow.links.reduce((sum, link) => sum + link.bps, 0)).toBe(report.nodes.reduce((sum, node) => sum + node.used, 0));
  expect(flow.links[0].source).toBe(report.nodes[0].rack);
});
