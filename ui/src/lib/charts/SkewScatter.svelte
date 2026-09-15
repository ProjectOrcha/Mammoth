<!-- SkewScatter: one point per partition. x = size, y = reads. The outlier in
     the top right is the task that sets your job's runtime, and this is the
     fastest way to find it. Log scale on x, because skew is multiplicative. -->
<script lang="ts">
  import { escapeHtml } from '$lib/html';
  import type { SkewReport } from '$lib/types';
  import { bytes, count } from '$lib/format';
  import { chart, palette, tooltipStyle, type ChartOption } from './echarts.svelte';

  interface Props {
    report: SkewReport;
  }

  let { report }: Props = $props();

  const option = $derived((): ChartOption => {
    const p = palette();
    const median = report.median;
    const hasReads = report.points.some((point) => point.reads !== null);

    // A log axis cannot plot zero. Keep the original size for the tooltip;
    // empty files share the <=1 B bucket instead of silently disappearing.
    const points = report.points.map((pt, index) => [Math.max(1, pt.size), hasReads ? Math.max(0, pt.reads ?? 0) : index + 1, pt.partition, pt.writes, pt.size]);
    const hot = points.filter((d) => median > 0 && (d[4] as number) > median * 8);
    const normal = points.filter((d) => median <= 0 || (d[4] as number) <= median * 8);

    return {
      backgroundColor: 'transparent',
      grid: { left: 56, right: 20, top: 18, bottom: 40 },
      tooltip: {
        ...tooltipStyle(p),
        formatter: (params: unknown) => {
          const d = (params as { value: [number, number, string, number | null, number] }).value;
          return [
            `<b>${escapeHtml(d[2])}</b>`,
            `${bytes(d[4])}${median > 0 ? ` · ${(d[4] / median).toFixed(1)}× median` : ''}`,
            hasReads ? `${count(d[1])} reads${d[3] === null ? '' : ` · ${count(d[3])} writes`}` : 'File size · read counts are not collected',
          ].join('<br/>');
        },
      },
      xAxis: {
        type: 'log',
        name: 'file size',
        nameLocation: 'middle',
        nameGap: 26,
        nameTextStyle: { color: p.faint, fontFamily: p.fontMono, fontSize: 10 },
        axisLabel: {
          color: p.faint,
          fontFamily: p.fontMono,
          fontSize: 10,
          formatter: (v: number) => v === 1 ? '≤1 B' : bytes(v, 0),
        },
        axisLine: { lineStyle: { color: p.rule } },
        splitLine: { lineStyle: { color: p.rule, opacity: 0.25 } },
      },
      yAxis: {
        type: 'value',
        minInterval: 1,
        name: hasReads ? 'reads · 7d' : 'file number',
        nameLocation: 'middle',
        nameGap: 40,
        nameTextStyle: { color: p.faint, fontFamily: p.fontMono, fontSize: 10 },
        axisLabel: {
          color: p.faint,
          fontFamily: p.fontMono,
          fontSize: 10,
          formatter: (v: number) => count(v),
        },
        axisLine: { lineStyle: { color: p.rule } },
        splitLine: { lineStyle: { color: p.rule, opacity: 0.25 } },
      },
      series: [
        {
          type: 'scatter',
          name: 'partitions',
          symbolSize: 7,
          data: normal,
          itemStyle: { color: p.accent, opacity: 0.55 },
          markLine: {
            silent: true,
            symbol: 'none',
            label: {
              color: p.faint,
              fontFamily: p.fontMono,
              fontSize: 9,
              formatter: 'median',
            },
            lineStyle: { color: p.rule, type: 'dashed' },
            data: median > 0 ? [{ xAxis: median }] : [],
          },
        },
        {
          type: 'scatter',
          name: 'skewed',
          symbolSize: 14,
          data: hot,
          itemStyle: { color: p.danger },
          label: {
            show: false,
            position: 'left',
            distance: 10,
            color: p.danger,
            fontFamily: p.fontMono,
            fontSize: 10,
            formatter: (params: unknown) =>
              (params as { value: [number, number, string] }).value[2],
          },
        },
      ],
    };
  });
</script>

<div class="chart" role="img" aria-label={`Size distribution of ${report.files} files`} use:chart={option}></div>
{#if report.points.some(point => point.reads === null)}
  <p class="quiet">Each point is a file. Read counts are not collected in local storage.</p>
{/if}
<details>
  <summary>View file sizes</summary>
  <table>
    <thead><tr><th>File</th><th class="num">Size</th></tr></thead>
    <tbody>{#each report.points as point (point.partition)}<tr><td class="file">{point.partition}</td><td class="num">{bytes(point.size)}</td></tr>{/each}</tbody>
  </table>
</details>

<style>
  .chart {
    width: 100%;
    height: 19rem;
  }
  .quiet { color: var(--fg-faint); font-size: .75rem; }
  summary { color: var(--accent); cursor: pointer; }
  .file { white-space: normal; overflow-wrap: anywhere; }
</style>
