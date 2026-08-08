<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { IconChevronDown, IconPlayerPlay, IconRefresh } from "@tabler/icons-svelte";
  import type { AppConfig, InputDevice } from "$lib/types";
  import Meter from "../Meter.svelte";
  import { pipelineState } from "$lib/stores/pipeline";

  let {
    config = $bindable(),
  }: {
    config: AppConfig;
  } = $props();

  let devices = $state<InputDevice[]>([]);
  let deviceError = $state<string | null>(null);
  let loadingDevices = $state(false);

  async function loadDevices() {
    loadingDevices = true;
    deviceError = null;
    try {
      devices = await invoke<InputDevice[]>("list_audio_inputs");
    } catch (error) {
      devices = [];
      deviceError = String(error);
    } finally {
      loadingDevices = false;
    }
  }

  async function previewCue(cue: "start" | "stop" | "done" | "error") {
    try {
      await invoke("preview_sound_cue", { cue, config: config.sounds });
    } catch (error) {
      console.error("cue preview failed", error);
    }
  }

  // The saved device may no longer exist (unplugged headset). Say so rather than
  // silently showing "System default", which would misreport what is stored.
  let missingDevice = $derived(
    config.audio.input_device !== null &&
      devices.length > 0 &&
      !devices.some((device) => device.name === config.audio.input_device),
  );

  const cues = [
    { id: "start", label: "Start", key: "on_start" },
    { id: "stop", label: "Stop", key: "on_stop" },
    { id: "done", label: "Inserted", key: "on_done" },
    { id: "error", label: "Error", key: "on_error" },
  ] as const;

  onMount(() => {
    void loadDevices();
  });
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Audio</h2>
    <p class="section__lead">
      Which microphone Oto listens to, how the signal is treated on the way to the
      speech engine, and when a hands-free session decides you are finished.
    </p>
  </header>

  <!-- Gain and gate are set by ear and by eye, so the meter belongs here at full
       size rather than only in the rail. -->
  <div class="bench">
    <div class="bench__head">
      <span class="plate-micro">Live input</span>
      <span class="bench__state">
        {$pipelineState === "listening" ? "Recording" : "Silent until you dictate"}
      </span>
    </div>
    <Meter segments={40} variant="tall" />
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">
        Input
        <button type="button" class="btn btn--small" disabled={loadingDevices} onclick={() => void loadDevices()}>
          <IconRefresh aria-hidden="true" size={13} stroke={1.8} />
          {loadingDevices ? "Scanning…" : "Rescan"}
        </button>
      </span>
      <p class="rack__note">
        If the chosen device disappears, Oto falls back to your system default.
      </p>
    </div>

    <label class="row">
      <span class="row__label">Microphone</span>
      <span class="row__control select-wrap">
        <select
          value={config.audio.input_device ?? ""}
          onchange={(event) => {
            config.audio.input_device = event.currentTarget.value || null;
          }}
        >
          <option value="">System default</option>
          {#each devices as device (device.name)}
            <option value={device.name}>
              {device.name}{device.is_default ? " — system default" : ""}
            </option>
          {/each}
          {#if missingDevice}
            <option value={config.audio.input_device}>
              {config.audio.input_device} (not connected)
            </option>
          {/if}
        </select>
        <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
        {#if deviceError}
          <span class="row__hint status-warn">Could not list devices ({deviceError}).</span>
        {:else if missingDevice}
          <span class="row__hint status-warn">
            Not connected right now. Oto records from the system default until it returns.
          </span>
        {/if}
      </span>
    </label>

    <label class="row">
      <span class="row__label">Gain</span>
      <span class="row__control">
        <span class="slider-head">
          <span class="row__hint">Raise it for a quiet microphone.</span>
          <span class="slider-value">{config.audio.input_gain.toFixed(2)}×</span>
        </span>
        <input type="range" min="0.25" max="4" step="0.05" bind:value={config.audio.input_gain} />
        <span class="row__hint">
          Loud samples clip rather than wrap, so too much gain distorts instead of crackling. Watch
          the meter above stay out of the red while you talk.
        </span>
      </span>
    </label>

    <label class="row row--switch row--flush">
      <span class="row__copy">
        <strong>Noise gate</strong>
        <span>
          Quietens audio sitting at the level of your room. Good against fans and keyboards; the
          threshold learns your room on its own.
        </span>
      </span>
      <input type="checkbox" bind:checked={config.audio.noise_gate} />
    </label>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Activation</span>
      <p class="rack__note">How the shortcut starts and ends a dictation.</p>
    </div>

    <div class="row row--stacked">
      <span class="row__label">Mode</span>
      <div class="row__control choice-list">
        {#each [
          {
            value: "hold",
            title: "Hold to talk",
            detail: "Hold the chord while you speak; releasing it transcribes.",
          },
          {
            value: "toggle",
            title: "Toggle",
            detail: "Press once to start and again to stop. Hands free in between.",
          },
          {
            value: "hybrid",
            title: "Hybrid",
            detail: "A quick tap toggles; holding past the threshold behaves like push-to-talk.",
          },
        ] as const as option (option.value)}
          <label class="choice" data-active={config.activation_mode === option.value}>
            <input
              type="radio"
              name="activation-mode"
              value={option.value}
              bind:group={config.activation_mode}
            />
            <span class="choice__copy">
              <strong>{option.title}</strong>
              <span>{option.detail}</span>
            </span>
          </label>
        {/each}
      </div>
    </div>

    {#if config.activation_mode === "hybrid"}
      <label class="row row--flush">
        <span class="row__label">Tap threshold</span>
        <span class="row__control">
          <span class="slider-head">
            <span class="row__hint">Releases quicker than this count as a tap.</span>
            <span class="slider-value">{config.hybrid_tap_threshold_ms} ms</span>
          </span>
          <input type="range" min="120" max="1000" step="10" bind:value={config.hybrid_tap_threshold_ms} />
        </span>
      </label>
    {/if}
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Silence</span>
      <p class="rack__note">
        Ends a hands-free session once you stop talking. A held chord always ends on its own
        release, whatever this says.
      </p>
    </div>

    <label class="row row--switch" class:row--flush={!config.vad.auto_stop}>
      <span class="row__copy">
        <strong>Stop when I stop talking</strong>
        <span>Applies to Toggle, and to taps in Hybrid.</span>
      </span>
      <input type="checkbox" bind:checked={config.vad.auto_stop} />
    </label>

    {#if config.vad.auto_stop}
      <label class="row">
        <span class="row__label">Trailing silence</span>
        <span class="row__control">
          <span class="slider-head">
            <span class="row__hint">Shorter feels quicker but can cut you off mid-thought.</span>
            <span class="slider-value">{(config.vad.silence_ms / 1000).toFixed(1)} s</span>
          </span>
          <input type="range" min="400" max="5000" step="100" bind:value={config.vad.silence_ms} />
        </span>
      </label>

      <label class="row row--flush">
        <span class="row__label">Minimum speech</span>
        <span class="row__control">
          <span class="slider-head">
            <span class="row__hint">Stops a cough or a slow start from ending the session.</span>
            <span class="slider-value">{config.vad.min_speech_ms} ms</span>
          </span>
          <input type="range" min="0" max="2000" step="50" bind:value={config.vad.min_speech_ms} />
        </span>
      </label>
    {/if}
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Cues</span>
      <p class="rack__note">
        Short tones for each transition — worth having when the overlay is hidden or you are
        looking at another window.
      </p>
    </div>

    <label class="row row--switch" class:row--flush={!config.sounds.enabled}>
      <span class="row__copy">
        <strong>Play cues</strong>
        <span>Synthesised as they play. No audio files involved.</span>
      </span>
      <input type="checkbox" bind:checked={config.sounds.enabled} />
    </label>

    {#if config.sounds.enabled}
      <label class="row">
        <span class="row__label">Volume</span>
        <span class="row__control">
          <span class="slider-head">
            <span class="row__hint">Relative to your system output.</span>
            <span class="slider-value">{Math.round(config.sounds.volume * 100)}%</span>
          </span>
          <input type="range" min="0" max="1" step="0.05" bind:value={config.sounds.volume} />
        </span>
      </label>

      <div class="row row--stacked row--flush">
        <span class="row__label">Play on</span>
        <div class="row__control cues">
          {#each cues as cue (cue.id)}
            <div class="cue">
              <label class="cue__toggle">
                <input type="checkbox" bind:checked={config.sounds[cue.key]} />
                <span>{cue.label}</span>
              </label>
              <button
                type="button"
                class="btn btn--small"
                onclick={() => void previewCue(cue.id)}
              >
                <IconPlayerPlay aria-hidden="true" size={12} stroke={1.8} />
                Hear it
              </button>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <p class="note">
    After changing anything here, run <strong>Permissions → Test microphone</strong>. The test
    records through this same device, gain and gate, so a test that passes means dictation will
    hear you too.
  </p>
</section>

<style>
  .bench {
    display: grid;
    gap: 0.5rem;
    padding: 0.75rem 0.875rem 0.875rem;
    border: var(--rule) solid var(--etch);
    border-radius: var(--radius-panel);
    background: var(--well);
  }

  .bench__head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-sm);
    color: var(--faint);
  }

  .bench__state {
    color: var(--muted);
    font-size: var(--text-xs);
  }

  .cues {
    gap: 2px;
  }

  .cue {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-sm);
    padding: 0.4375rem 0.625rem;
    border: var(--rule) solid var(--etch);
    border-radius: var(--radius-control);
    background: var(--panel);
  }

  .cue__toggle {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    flex: 1;
    color: var(--ink-2);
    font-size: var(--text-sm);
  }
</style>
