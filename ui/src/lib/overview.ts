import type { ClusterReport, NodeReport, NodeState } from './types';
import { pctValue } from './format';

/** Keep the headline and the detailed health breakdown on the same denominator. */
export function summarizeCluster(report: ClusterReport) {
  const totalBlocks = Object.values(report.health).reduce(
    (sum, value) => sum + value,
    0,
  );
  const atRisk =
    report.health.critical + report.health.corrupt + report.health.missing;
  const healthyNodes = report.nodes.filter(
    (node) => node.state === 'healthy',
  ).length;
  const deadNodes = report.nodes.filter((node) => node.state === 'dead').length;
  const needsAttention =
    report.safe_mode ||
    report.nodes.length === 0 ||
    atRisk > 0 ||
    healthyNodes < report.nodes.length ||
    report.health.under_replicated > 0 ||
    report.alerts.some((alert) => alert.level !== 'info');
  return { totalBlocks, atRisk, healthyNodes, deadNodes, needsAttention };
}

const priority: Record<NodeState, number> = {
  dead: 4,
  warn: 3,
  decommissioning: 2,
  maintenance: 1,
  healthy: 0,
};

/** State takes priority over fullness, so a slow worker cannot hide below healthy ones. */
export function priorityNodes(nodes: NodeReport[], limit = 6): NodeReport[] {
  return [...nodes]
    .sort(
      (a, b) =>
        priority[b.state] - priority[a.state] ||
        pctValue(b.used, b.capacity) - pctValue(a.used, a.capacity),
    )
    .slice(0, limit);
}
