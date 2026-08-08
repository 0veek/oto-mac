<script lang="ts">
  import { onMount } from "svelte";
  import { audioLevel } from "$lib/stores/pipeline";

  let {
    level,
    segments = 24,
    variant,
    onlevel,
  }: {
    /** Fixed level for previews. Omit to follow the live microphone. */
    level?: number;
    segments?: number;
    variant?: "compact" | "tall";
    /** Reports the ballistics-smoothed level each frame, for a dB readout. */
    onlevel?: (value: number) => void;
  } = $props();

  let shown = $state(0);
  let peak = $state(0);

  function clamp(value: number) {
    return Math.max(0, Math.min(1, Number.isFinite(value) ? value : 0));
  }

  onMount(() => {
    let frame = 0;
    let target = 0;
    let peakSetAt = 0;
    let phase = 0;

    // Live mode follows the pipeline; preview mode animates around a fixed level
    // so a screenshot of the overlay is not a frozen ladder.
    const unsubscribe =
      level === undefined
        ? audioLevel.subscribe((value) => {
            target = clamp(value);
          })
        : () => {};

    const tick = () => {
      if (level !== undefined) {
        phase += 1;
        target = clamp(level + Math.sin(phase / 9) * 0.16);
      }

      // Real meter ballistics: instant attack so a syllable reads as a spike,
      // slow release so silence falls away instead of snapping to nothing.
      shown = target > shown ? target : shown * 0.86 + target * 0.14;
      if (shown < 0.002) shown = 0;

      const now = performance.now();
      if (shown >= peak) {
        peak = shown;
        peakSetAt = now;
      } else if (now - peakSetAt > 900) {
        peak = Math.max(shown, peak - 0.01);
      }

      onlevel?.(shown);
      frame = requestAnimationFrame(tick);
    };

    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
      unsubscribe();
    };
  });

  const litCount = $derived(Math.round(shown * segments));
  const peakIndex = $derived(
    peak > 0.02 ? Math.min(segments - 1, Math.round(peak * segments) - 1) : -1,
  );

  /** Meter zones read like hardware: green headroom, amber hot, red clipping. */
  function zone(index: number) {
    const ratio = (index + 1) / segments;
    if (ratio > 0.88) return "clip";
    if (ratio > 0.62) return "hot";
    return "safe";
  }
</script>

<div
  class="meter"
  class:meter--compact={variant === "compact"}
  class:meter--tall={variant === "tall"}
  aria-hidden="true"
>
  {#each { length: segments } as _, index (index)}
    <span
      class="meter__seg"
      data-lit={index < litCount ? zone(index) : undefined}
      data-peak={index === peakIndex && index >= litCount ? "true" : undefined}
    ></span>
  {/each}
</div>
