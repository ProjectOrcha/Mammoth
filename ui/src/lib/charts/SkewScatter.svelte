<!-- SkewScatter: one point per partition. x = size, y = reads. The outlier in
     the top right is the task that sets your job's runtime, and this is the
     fastest way to find it. Log scale on x, because skew is multiplicative. -->
<script lang="ts">
  import type { SkewReport } from '$lib/types';
  import { bytes, count } from '$lib/format';
  import { chart, palette, tooltipStyle, type ChartOption } from './echarts';

  interface Props {
    report: SkewReport;
  }

  let { report }: Props = $props();

  const option = $derived((): ChartOption => {
    const p = palette();
    const median = report.median;

    const points = report.points.map((pt) => [pt.size, pt.reads, pt.partition, pt.writes]);
    const hot = points.filter((d) => (d[0] as number) > median * 8);
    const normal = points.filter((d) => (d[0] as number) <= median * 8);

    return {
      backgroundColor: 'transparent',
      grid: { left: 56, right: 20, top: 18, bottom: 40 },
      tooltip: {
        ...tooltipStyle(p),
        formatter: (params: unknown) => {
          const d = (params as { value: [number, number, string, number] }).value;
          return [
            `<b>${d[2]}</b>`,
            `${bytes(d[0])} · ${(d[0] / median).toFixed(1)}× median`,
            `${count(d[1])} reads · ${count(d[3])} writes`,
          ].join('<br/>');
        },
      },
      xAxis: {
        type: 'log',
        name: 'partition size',
        nameLocation: 'middle',
        nameGap: 26,
        nameTextStyle: { color: p.faint, fontFamily: p.fontMono, fontSize: 10 },
        axisLabel: {
          color: p.faint,
          fontFamily: p.fontMono,
          fontSize: 10,
          formatter: (v: number) => bytes(v, 0),
        },
        axisLine: { lineStyle: { color: p.rule } },
        splitLine: { lineStyle: { color: p.rule, opacity: 0.25 } },
      },
      yAxis: {
        type: 'log',
        name: 'reads · 7d',
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
            data: [{ xAxis: median }],
          },
        },
        {
          type: 'scatter',
          name: 'skewed',
          symbolSize: 14,
          data: hot,
          itemStyle: { color: p.danger },
          label: {
            show: true,
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

<div class="chart" use:chart={option}></div>

<style>
  .chart {
    width: 100%;
    height: 19rem;
  }
</style>
