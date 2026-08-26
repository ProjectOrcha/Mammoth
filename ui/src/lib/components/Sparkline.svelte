<!-- An inline throughput trace. Deliberately unlabelled: it answers "is this
     steady, rising or dead", and the number next to it answers the rest. -->
<script lang="ts">
  interface Props {
    points: number[];
    width?: number;
    height?: number;
    tone?: string;
  }

  let { points, width = 78, height = 18, tone = 'var(--fg-faint)' }: Props = $props();

  const d = $derived.by(() => {
    if (points.length < 2) return '';
    const max = Math.max(...points, 1);
    const step = width / (points.length - 1);
    return points
      .map((p, i) => `${i === 0 ? 'M' : 'L'}${(i * step).toFixed(1)},${(height - (p / max) * height).toFixed(1)}`)
      .join(' ');
  });
</script>

<svg {width} {height} viewBox="0 0 {width} {height}" aria-hidden="true" class="spark">
  {#if d}<path {d} fill="none" stroke={tone} stroke-width="1.25" vector-effect="non-scaling-stroke" />{/if}
</svg>

<style>
  .spark {
    display: block;
    overflow: visible;
  }
</style>
