<!-- A usage bar. Colour comes from the value, not from the caller, so 94% is
     the same red everywhere it appears. -->
<script lang="ts">
  interface Props {
    value: number;
    max?: number;
    /** Override the automatic colour ramp. */
    tone?: 'auto' | 'ok' | 'warn' | 'danger' | 'accent';
    height?: string;
    label?: string;
  }

  let { value, max = 100, tone = 'auto', height = '0.5rem', label }: Props = $props();

  const fraction = $derived(Math.max(0, Math.min(1, max ? value / max : 0)));
  const resolved = $derived(
    tone !== 'auto' ? tone : fraction >= 0.9 ? 'danger' : fraction >= 0.75 ? 'warn' : 'ok',
  );
</script>

<div
  class="meter"
  style="height: {height}"
  role="meter"
  aria-valuenow={value}
  aria-valuemin={0}
  aria-valuemax={max}
  aria-label={label ?? 'usage'}
>
  <div class="fill" data-tone={resolved} style="width: {fraction * 100}%"></div>
</div>

<style>
  .meter {
    background: var(--track);
    width: 100%;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    transition: width 0.5s ease;
  }
  .fill[data-tone='ok'] {
    background: var(--ok);
  }
  .fill[data-tone='warn'] {
    background: var(--warn);
  }
  .fill[data-tone='danger'] {
    background: var(--danger);
  }
  .fill[data-tone='accent'] {
    background: var(--accent);
  }
</style>
