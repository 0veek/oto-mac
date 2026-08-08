<script lang="ts">
  import { IconAlertTriangle, IconCheck, IconLoader2, IconX } from "@tabler/icons-svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { PipelineState } from "$lib/types";
  import {
    partialTranscript,
    pipelineDetail,
    pipelinePhase,
    pipelineState,
  } from "$lib/stores/pipeline";
  import Meter from "./Meter.svelte";

  type InteractionState = "hover" | "focus" | "active";

  type PreviewState = {
    state: PipelineState;
    detail?: string;
    phase?: string;
    partial?: string;
    level?: number;
  };

  let {
    preview = null,
    forceInteraction,
    actionDisabled = false,
    actionBusy = false,
    onPreviewAction,
  }: {
    preview?: PreviewState | null;
    forceInteraction?: InteractionState;
    actionDisabled?: boolean;
    actionBusy?: boolean;
    onPreviewAction?: () => void;
  } = $props();

  let cancelBusy = $state(false);

  const currentState = $derived(preview?.state ?? $pipelineState);
  const detail = $derived(preview?.detail ?? $pipelineDetail);
  const phase = $derived(preview?.phase ?? $pipelinePhase);
  const partial = $derived(preview?.partial ?? $partialTranscript);
  const busy = $derived(actionBusy || cancelBusy);
  const hasAction = $derived(currentState !== "idle");
  const actionLabel = $derived(
    currentState === "error"
      ? "Dismiss error"
      : currentState === "done"
        ? "Dismiss confirmation"
        : "Cancel dictation",
  );

  async function handleAction() {
    if (busy || actionDisabled) return;
    if (preview) {
      onPreviewAction?.();
      return;
    }

    cancelBusy = true;
    try {
      await invoke("cancel_dictation");
    } catch (error) {
      console.error("cancel_dictation failed", error);
    } finally {
      cancelBusy = false;
    }
  }

  const PHASE_LABELS: Record<string, string> = {
    transcribing: "Transcribing…",
    polishing: "Polishing…",
    injecting: "Inserting…",
    "rewriting selection": "Rewriting…",
  };

  function statusLabel(value: PipelineState) {
    switch (value) {
      case "listening":
        return detail?.trim() || "Listening";
      case "processing": {
        if (phase) {
          return PHASE_LABELS[phase] || phase;
        }
        // Prefer backend detail (e.g. polish fallback toast) when present.
        return detail?.trim() || "Processing";
      }
      case "done":
        // Clipboard-only / inject path detail is the truth ("Copied — press ⌘V").
        return detail?.trim() || "Inserted";
      case "error":
        return detail?.trim() || "Couldn’t insert";
      default:
        return "Ready";
    }
  }

  const displayPartial = $derived(
    (currentState === "listening" || currentState === "processing") && partial.trim().length > 0
      ? partial.trim()
      : "",
  );
</script>

<div
  class:force-hover={forceInteraction === "hover"}
  class:force-focus={forceInteraction === "focus"}
  class:force-active={forceInteraction === "active"}
  class:is-disabled={actionDisabled}
  class:is-loading={busy}
  class="oto-pill state-{currentState}"
  role="status"
  aria-live="polite"
  aria-label={`Oto — ${statusLabel(currentState)}${displayPartial ? `. ${displayPartial}` : ""}`}
  title={detail || displayPartial || statusLabel(currentState)}
>
  <!-- A bare `data-tauri-drag-region` only fires on direct clicks on the element
       that carries it, so every child that covers the rail repeats the attribute
       — otherwise the capsule can only be dragged by its few pixels of padding. -->
  <div class="oto-pill__rail" data-tauri-drag-region>
    <!-- The lamp is the one thing visible in every state, so it carries status
         on its own when the capsule collapses to its idle square. -->
    <span
      class="oto-pill__lamp"
      data-state={currentState}
      data-tauri-drag-region
      aria-hidden="true"
    >
      {#if currentState === "done"}
        <IconCheck size={13} stroke={2.4} />
      {:else if currentState === "error"}
        <IconAlertTriangle size={13} stroke={2.2} />
      {/if}
    </span>

    {#if currentState === "listening" || currentState === "processing"}
      <span class="oto-pill__meter" data-tauri-drag-region aria-hidden="true">
        <Meter
          segments={7}
          variant="compact"
          level={preview ? (preview.level ?? 0.48) : undefined}
        />
      </span>
    {/if}

    {#key `${currentState}:${phase}:${displayPartial}:${detail}`}
      <span
        class:oto-pill__label--error={currentState === "error"}
        class="oto-pill__label"
        data-tauri-drag-region
      >
        {#if displayPartial}
          {displayPartial}
        {:else}
          {statusLabel(currentState)}
        {/if}
      </span>
    {/key}

    {#if hasAction}
      <button
        type="button"
        class="oto-pill__action"
        aria-label={actionLabel}
        title={actionLabel}
        disabled={actionDisabled || busy}
        onclick={handleAction}
      >
        {#if busy}
          <IconLoader2 class="oto-pill__spinner" size={14} stroke={2} aria-hidden="true" />
        {:else}
          <IconX size={14} stroke={2} aria-hidden="true" />
        {/if}
      </button>
    {/if}
  </div>
</div>

<style>
  /* The overlay is a small piece of equipment sitting on top of the desktop, so
     it keeps the chassis language: machined corners rather than a soft pill, a
     hairline edge, and a lit lamp instead of a coloured glow. */
  .oto-pill {
    width: min(15.75rem, calc(100vw - 0.5rem));
    height: min(2.5rem, calc(100vh - 0.5rem));
    color: var(--overlay-ink);
    font-family: var(--font-ui);
    user-select: none;
    transition:
      width var(--dur-throw) var(--ease-mech),
      height var(--dur-throw) var(--ease-mech);
  }

  /* Dormant mode: a square sitting quietly in the corner, lamp only. */
  .oto-pill.state-idle {
    width: min(2.5rem, calc(100vw - 0.5rem));
  }

  .oto-pill__rail {
    display: flex;
    width: 100%;
    height: 100%;
    box-sizing: border-box;
    align-items: center;
    gap: 0.5rem;
    padding: 0.25rem 0.25rem 0.25rem 0.625rem;
    border: var(--rule) solid var(--overlay-etch-strong);
    border-radius: 6px;
    background: var(--overlay-chassis);
    /* No drop shadow on macOS: the window itself is created with `shadow: false`,
       and a CSS shadow inside a transparent window paints a visible dark halo on
       the capsule's own corners rather than falling on the desktop behind it. */
    box-shadow: inset 0 1px 0 oklch(100% 0 0 / 0.07);
    backdrop-filter: blur(16px) saturate(115%);
    -webkit-backdrop-filter: blur(16px) saturate(115%);
  }

  .oto-pill.state-idle .oto-pill__rail {
    justify-content: center;
    gap: 0;
    padding: 0.25rem;
  }

  .oto-pill.state-idle .oto-pill__label {
    display: none;
  }

  .oto-pill__lamp {
    display: grid;
    width: 0.5rem;
    height: 0.5rem;
    flex: 0 0 auto;
    place-items: center;
    border-radius: var(--radius-round);
    background: var(--overlay-etch-strong);
    transition:
      width var(--dur-throw) var(--ease-mech),
      height var(--dur-throw) var(--ease-mech),
      background-color var(--dur-tick) var(--ease-lamp);
  }

  .oto-pill__lamp[data-state="listening"],
  .oto-pill__lamp[data-state="processing"] {
    background: var(--overlay-lamp);
    box-shadow: 0 0 0 2px oklch(82% 0.16 70 / 0.16);
  }

  /* Done and error carry a glyph, so the lamp grows into a proper badge. */
  .oto-pill__lamp[data-state="done"],
  .oto-pill__lamp[data-state="error"] {
    width: 1.125rem;
    height: 1.125rem;
    border-radius: 3px;
  }

  .oto-pill__lamp[data-state="done"] {
    color: oklch(18% 0.04 150);
    background: var(--overlay-safe);
  }

  .oto-pill__lamp[data-state="error"] {
    color: oklch(18% 0.04 27);
    background: var(--overlay-clip);
  }

  .oto-pill__meter {
    display: block;
    width: 2.25rem;
    flex: 0 0 2.25rem;
  }

  /* The overlay floats over arbitrary wallpaper and must stay dark even when the
     rest of the app is on the light chassis, so the ladder takes overlay tokens. */
  .oto-pill :global(.meter__seg) {
    background: var(--overlay-etch);
  }

  .oto-pill :global(.meter__seg[data-lit="safe"]) {
    background: var(--overlay-safe);
  }

  .oto-pill :global(.meter__seg[data-lit="hot"]) {
    background: var(--overlay-lamp);
  }

  .oto-pill :global(.meter__seg[data-lit="clip"]) {
    background: var(--overlay-clip);
  }

  .oto-pill :global(.meter__seg[data-peak="true"]) {
    background: var(--overlay-ink);
  }

  .oto-pill__label {
    min-width: 0;
    overflow: hidden;
    flex: 1 1 auto;
    color: var(--overlay-ink);
    font-size: 0.8125rem;
    font-weight: 540;
    letter-spacing: -0.004em;
    line-height: 1;
    text-overflow: ellipsis;
    white-space: nowrap;
    animation: pill-enter var(--dur-throw) var(--ease-mech) both;
  }

  .oto-pill__label--error {
    color: var(--overlay-clip);
  }

  .oto-pill__action {
    display: grid;
    width: 1.75rem;
    height: 1.75rem;
    min-height: 0;
    flex: 0 0 1.75rem;
    padding: 0;
    place-items: center;
    border: var(--rule) solid transparent;
    border-radius: var(--radius-control);
    outline: 2px solid transparent;
    outline-offset: 1px;
    color: var(--overlay-muted);
    background: transparent;
    transition:
      color var(--dur-tick) var(--ease-mech),
      background-color var(--dur-tick) var(--ease-mech),
      border-color var(--dur-tick) var(--ease-mech);
  }

  .oto-pill__action:hover,
  .force-hover .oto-pill__action {
    border-color: var(--overlay-etch);
    color: var(--overlay-ink);
    background: var(--overlay-raised);
  }

  .oto-pill__action:focus-visible,
  .force-focus .oto-pill__action {
    border-color: var(--overlay-lamp);
    outline-color: oklch(82% 0.16 70 / 0.45);
  }

  .oto-pill__action:active,
  .force-active .oto-pill__action {
    color: var(--overlay-ink);
    background: var(--overlay-raised-hot);
  }

  .oto-pill__action:disabled,
  .is-disabled .oto-pill__action {
    cursor: not-allowed;
    opacity: 0.4;
  }

  .oto-pill__spinner {
    animation: pill-spin 0.85s linear infinite;
  }

  .state-error .oto-pill__rail {
    border-color: oklch(68% 0.2 27 / 0.7);
  }

  .state-done .oto-pill__rail {
    border-color: oklch(78% 0.13 150 / 0.6);
  }

  @keyframes pill-enter {
    from {
      opacity: 0;
      transform: translateX(0.1875rem);
    }
  }

  @keyframes pill-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .oto-pill__label,
    .oto-pill__spinner {
      animation: none;
    }
  }

  :global(:root[data-reduce-motion="true"]) .oto-pill__label,
  :global(:root[data-reduce-motion="true"]) .oto-pill__spinner {
    animation: none;
  }
</style>
