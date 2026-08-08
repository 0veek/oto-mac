<script lang="ts">
  import { pipelineDetail, pipelinePhase, pipelineState } from "$lib/stores/pipeline";
  import Meter from "./Meter.svelte";

  let smoothed = $state(0);

  const live = $derived($pipelineState === "listening" || $pipelineState === "processing");

  const lit = $derived(
    $pipelineState === "error"
      ? "fault"
      : $pipelineState === "done"
        ? "ok"
        : live
          ? "live"
          : "off",
  );

  const PHASE_LABELS: Record<string, string> = {
    transcribing: "Transcribing",
    polishing: "Polishing",
    injecting: "Inserting",
    "rewriting selection": "Rewriting",
  };

  // Oto only samples the microphone during a session, so anything else is an
  // honest standby rather than a meter pretending to idle at silence.
  const stateLabel = $derived.by(() => {
    switch ($pipelineState) {
      case "listening":
        return "Listening";
      case "processing":
        return PHASE_LABELS[$pipelinePhase] ?? "Processing";
      case "done":
        return "Inserted";
      case "error":
        return $pipelineDetail.trim() || "Fault";
      default:
        return "Standby";
    }
  });

  /** Amplitude to dBFS, floored where the ladder bottoms out. */
  const readout = $derived.by(() => {
    if (!live) return "—";
    if (smoothed < 0.001) return "−∞";
    const db = 20 * Math.log10(smoothed);
    return db <= -60 ? "−∞" : `${db.toFixed(1)}`;
  });
</script>

<div class="gauge">
  <div class="gauge__head">
    <span class="plate-micro gauge__label">Input</span>
    <span class="readout gauge__value" data-live={live}>
      {readout}{#if live}<span class="gauge__unit">dBFS</span>{/if}
    </span>
  </div>

  <Meter onlevel={(value) => (smoothed = value)} />

  <div class="gauge__state">
    <span class="lamp" data-lit={lit === "off" ? undefined : lit}></span>
    <span class="plate-micro">{stateLabel}</span>
  </div>
</div>

<style>
  .gauge__unit {
    margin-inline-start: 0.3em;
    color: var(--faint);
    font-size: 0.85em;
  }

  .gauge__state span:last-child {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
