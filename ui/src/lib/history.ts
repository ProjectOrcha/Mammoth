import type { ClusterReport, FlowReport, HeatCell, TopologyReport } from './types';

export interface Snapshot { at: number; report: ClusterReport }
export function recordSnapshot(history: Snapshot[], report: ClusterReport, at: number): Snapshot[] {
  if (!report.capabilities?.local || (history.length && at - history[history.length - 1].at < 10_000)) return history;
  // Thirty minutes, sampled every ten seconds. No file contents are retained.
  return [...history.slice(-180), { at, report }];
}
export function snapshotHeat(report: ClusterReport): HeatCell[] {
  return report.nodes.map(node => ({ node: node.id, rack: node.rack, state: node.state, usage: node.capacity > 0 ? node.used / node.capacity * 100 : 0, fragments: node.fragments, read_qps: Number.NaN, write_qps: Number.NaN, disk_p99_ms: node.disk_p99_ms }));
}
export function snapshotTopology(report: ClusterReport): TopologyReport {
  return { epoch: report.topology_epoch, nodes: report.nodes, racks: [...new Set(report.nodes.map(node => node.rack))], links: [] };
}
/** Stored bytes per rack and worker, not network throughput. */
export function replicaFlow(report: ClusterReport): FlowReport {
  return { window_s: 0, nodes: [...new Set(report.nodes.flatMap(node => [node.rack, node.id]))], links: report.nodes.filter(node => node.used > 0).map(node => ({ source: node.rack, target: node.id, bps: node.used })) };
}
