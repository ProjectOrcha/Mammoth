// One ECharts setup for the whole app.
//
// Imported from `echarts/core` with only the four chart types we actually draw,
// because the full bundle is about a megabyte and this UI is compiled into the
// `mammoth` binary. Adding a chart type means adding it to the `use()` call
// below and nowhere else.

import * as echarts from 'echarts/core';
import { GraphChart, SankeyChart, ScatterChart, TreemapChart } from 'echarts/charts';
import {
  GridComponent,
  LegendComponent,
  MarkLineComponent,
  TooltipComponent,
  VisualMapComponent,
} from 'echarts/components';
import { CanvasRenderer } from 'echarts/renderers';
import type { ComposeOption } from 'echarts/core';
import type {
  GraphSeriesOption,
  SankeySeriesOption,
  ScatterSeriesOption,
  TreemapSeriesOption,
} from 'echarts/charts';
import type {
  GridComponentOption,
  LegendComponentOption,
  TooltipComponentOption,
  VisualMapComponentOption,
} from 'echarts/components';
import type { Action } from 'svelte/action';

/** Only the pieces registered below. Importing `EChartsOption` from the package
 *  root would pull in every series type — and that entry point is a `export =`
 *  declaration, which an ES module cannot import cleanly. */
export type ChartOption = ComposeOption<
  | GraphSeriesOption
  | SankeySeriesOption
  | ScatterSeriesOption
  | TreemapSeriesOption
  | GridComponentOption
  | LegendComponentOption
  | TooltipComponentOption
  | VisualMapComponentOption
>;

echarts.use([
  GraphChart,
  SankeyChart,
  ScatterChart,
  TreemapChart,
  GridComponent,
  LegendComponent,
  MarkLineComponent,
  TooltipComponent,
  VisualMapComponent,
  CanvasRenderer,
]);

export interface Palette {
  fg: string;
  dim: string;
  faint: string;
  rule: string;
  panel: string;
  plate: string;
  accent: string;
  display: string;
  ok: string;
  warn: string;
  danger: string;
  info: string;
  fontMono: string;
}

/** Read the live theme out of CSS custom properties, so the charts follow the
 *  light/dark toggle instead of carrying a second copy of the palette. */
export function palette(): Palette {
  const s = getComputedStyle(document.documentElement);
  const v = (name: string) => s.getPropertyValue(name).trim();
  return {
    fg: v('--fg'),
    dim: v('--fg-dim'),
    faint: v('--fg-faint'),
    rule: v('--rule-strong'),
    panel: v('--bg-panel'),
    plate: v('--bg-plate'),
    accent: v('--accent'),
    display: v('--fg-display'),
    ok: v('--ok'),
    warn: v('--warn'),
    danger: v('--danger'),
    info: v('--info'),
    fontMono: v('--font-mono'),
  };
}

/** The tooltip styling every chart shares. */
export function tooltipStyle(p: Palette) {
  return {
    backgroundColor: p.plate,
    borderColor: p.rule,
    borderWidth: 1,
    textStyle: { color: p.fg, fontSize: 11, fontFamily: p.fontMono },
    extraCssText: 'border-radius: 0; box-shadow: none;',
  };
}

/**
 * `use:chart={() => option}` — the option is a thunk so the action can re-run
 * it after a theme change without the caller wiring that up. Handles resize
 * and disposal, which are the two things every hand-rolled ECharts wrapper
 * forgets.
 */
export const chart: Action<HTMLElement, () => ChartOption> = (node, build) => {
  let instance = echarts.init(node, undefined, { renderer: 'canvas' });
  let make = build;

  const draw = () => instance.setOption(make(), { notMerge: true });
  draw();

  const ro = new ResizeObserver(() => instance.resize());
  ro.observe(node);

  // The palette lives in CSS variables, so a theme flip has to be re-read
  // rather than re-computed from a JS constant.
  const mo = new MutationObserver(draw);
  mo.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });

  return {
    update(next: () => ChartOption) {
      make = next;
      draw();
    },
    destroy() {
      ro.disconnect();
      mo.disconnect();
      instance.dispose();
    },
  };
};
