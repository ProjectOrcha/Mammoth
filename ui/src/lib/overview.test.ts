import { describe, expect, it } from 'vitest';
import { clusterReport } from './demo';
import { priorityNodes, summarizeCluster } from './overview';

describe('overview summaries', () => {
  it('includes every health category and flags corrupt or missing blocks', () => {
    const report = clusterReport();
    report.health = {
      healthy: 80,
      under_replicated: 5,
      critical: 2,
      over_replicated: 10,
      corrupt: 1,
      missing: 2,
    };
    const summary = summarizeCluster(report);
    expect(summary.totalBlocks).toBe(100);
    expect(summary.atRisk).toBe(5);
    expect(summary.needsAttention).toBe(true);
  });

  it('does not call an empty cluster healthy', () => {
    const report = clusterReport();
    report.nodes = [];
    report.alerts = [];
    report.health = {
      healthy: 0,
      under_replicated: 0,
      critical: 0,
      over_replicated: 0,
      corrupt: 0,
      missing: 0,
    };
    expect(summarizeCluster(report)).toMatchObject({
      healthyNodes: 0,
      totalBlocks: 0,
      needsAttention: true,
    });
  });

  it('shows unhealthy workers before fuller healthy workers without changing the report', () => {
    const template = clusterReport().nodes[0];
    const nodes = [
      {
        ...template,
        id: 'healthy-full',
        state: 'healthy' as const,
        used: 100,
        capacity: 100,
      },
      {
        ...template,
        id: 'warning-empty',
        state: 'warn' as const,
        used: 0,
        capacity: 0,
      },
      { ...template, id: 'dead', state: 'dead' as const, used: 0, capacity: 0 },
      {
        ...template,
        id: 'warning-full',
        state: 'warn' as const,
        used: 90,
        capacity: 100,
      },
    ];
    expect(priorityNodes(nodes, 3).map((node) => node.id)).toEqual([
      'dead',
      'warning-full',
      'warning-empty',
    ]);
    expect(nodes[0].id).toBe('healthy-full');
  });
});
