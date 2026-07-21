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
  import Waveform from "./Waveform.svelte";

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

  function statusLabel(value: PipelineState) {
    switch (value) {
      case "listening":
        return "Listening";
      case "processing":
        return phase || "Processing";
      case "done":
        return "Inserted";
      case "error":
        return detail || "Couldn’t insert";
      default:
        return "Ready";
    }
  }
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
  aria-label={`Oto — ${statusLabel(currentState)}${partial ? `. ${partial}` : ""}`}
  title={detail || partial || statusLabel(currentState)}
>
  <div class="oto-pill__glass" data-tauri-drag-region>
    <span class="oto-pill__signal" data-tauri-drag-region aria-hidden="true">
      <Waveform level={preview?.level ?? (currentState === "processing" ? 0.55 : undefined)} />
    </span>

    <span class:oto-pill__label--error={currentState === "error"} class="oto-pill__label" data-tauri-drag-region>
      {statusLabel(currentState)}
    </span>

    <span class="oto-pill__status" data-tauri-drag-region aria-hidden="true">
      {#if currentState === "done"}
        <IconCheck size={13} stroke={2.35} />
      {:else if currentState === "error"}
        <IconAlertTriangle size={13} stroke={2.2} />
      {:else}
        <i></i><i></i><i></i>
      {/if}
    </span>

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
          <IconLoader2 class="oto-pill__spinner" size={13} stroke={2} aria-hidden="true" />
        {:else}
          <IconX size={14} stroke={2} aria-hidden="true" />
        {/if}
      </button>
    {/if}
  </div>
</div>

<style>
  .oto-pill {
    position: relative;
    width: min(13.75rem, 100vw);
    height: min(2.25rem, 100vh);
    color: var(--color-overlay-ink);
    font-family: var(--font-body);
    user-select: none;
    isolation: isolate;
  }

  .oto-pill__glass {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    min-width: 0;
    gap: 0.5rem;
    padding: 0.1875rem 0.1875rem 0.1875rem 0.65rem;
    overflow: hidden;
    border: 1px solid var(--color-overlay-rule);
    border-radius: 1.125rem;
    background: var(--color-overlay-surface);
    box-shadow:
      inset 0 1px 0 rgb(255 255 255 / 0.13),
      inset 0 -1px 0 rgb(0 0 0 / 0.2);
    backdrop-filter: blur(22px) saturate(135%);
    -webkit-backdrop-filter: blur(22px) saturate(135%);
  }

  .oto-pill__signal {
    display: grid;
    width: 1.75rem;
    flex: 0 0 1.75rem;
    place-items: center;
    color: var(--color-overlay-ink-2);
  }

  .oto-pill__label {
    min-width: 0;
    max-width: 5.3rem;
    overflow: hidden;
    flex: 1 1 auto;
    color: var(--color-overlay-ink);
    font-size: 0.75rem;
    font-weight: 570;
    letter-spacing: -0.006em;
    line-height: 1;
    text-overflow: ellipsis;
    white-space: nowrap;
    animation: oto-state-enter var(--dur-short) var(--ease-out) both;
  }

  .oto-pill__label--error {
    color: var(--color-overlay-error);
  }

  .oto-pill__status {
    display: flex;
    min-width: 1.75rem;
    align-items: center;
    justify-content: center;
    gap: 0.2rem;
    color: var(--color-overlay-accent);
  }

  .oto-pill__status i {
    display: block;
    width: 0.22rem;
    height: 0.22rem;
    border-radius: 50%;
    background: currentColor;
    animation: oto-processing 0.9s var(--ease-in-out) infinite alternate;
  }

  .oto-pill__status i:nth-child(2) {
    animation-delay: 120ms;
  }

  .oto-pill__status i:nth-child(3) {
    animation-delay: 240ms;
  }

  .state-idle .oto-pill__status {
    color: var(--color-overlay-muted);
  }

  .state-error .oto-pill__status {
    color: var(--color-overlay-error);
  }

  .oto-pill__action {
    display: grid;
    width: 1.75rem;
    height: 1.75rem;
    flex: 0 0 1.75rem;
    place-items: center;
    padding: 0;
    border: 1px solid var(--color-overlay-rule-strong);
    border-radius: 50%;
    outline: 2px solid transparent;
    outline-offset: 1px;
    color: var(--color-overlay-ink-2);
    background: var(--color-overlay-action);
    transition:
      color var(--dur-micro) var(--ease-out),
      background-color var(--dur-micro) var(--ease-out),
      border-color var(--dur-micro) var(--ease-out),
      transform var(--dur-micro) var(--ease-out),
      opacity var(--dur-micro) var(--ease-out);
  }

  .oto-pill__action:hover,
  .force-hover .oto-pill__action {
    border-color: var(--color-overlay-rule-hover);
    color: var(--color-overlay-ink);
    background: var(--color-overlay-action-hover);
  }

  .oto-pill__action:focus-visible,
  .force-focus .oto-pill__action {
    border-color: var(--color-overlay-accent);
    outline-color: var(--color-overlay-focus);
  }

  .oto-pill__action:active,
  .force-active .oto-pill__action {
    transform: scale(0.9);
    background: var(--color-overlay-action-active);
  }

  .oto-pill__action:disabled,
  .is-disabled .oto-pill__action {
    cursor: not-allowed;
    opacity: 0.42;
  }

  .state-error .oto-pill__glass {
    border-color: var(--color-overlay-error-rule);
  }

  .state-done .oto-pill__glass {
    border-color: var(--color-overlay-accent-rule);
  }

  .oto-pill__spinner {
    animation: oto-spin 0.85s linear infinite;
  }

  @keyframes oto-state-enter {
    from {
      opacity: 0;
      transform: translateX(0.2rem);
    }
  }

  @keyframes oto-processing {
    from {
      opacity: 0.25;
      transform: scale(0.7);
    }
    to {
      opacity: 0.9;
      transform: scale(1);
    }
  }

  @keyframes oto-spin {
    to {
      transform: rotate(360deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .oto-pill *,
    .oto-pill *::before,
    .oto-pill *::after {
      animation-duration: 1ms !important;
      animation-iteration-count: 1 !important;
      transition-duration: 1ms !important;
    }
  }
</style>
