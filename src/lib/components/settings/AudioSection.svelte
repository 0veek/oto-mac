<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { IconMicrophone, IconPlayerPlay, IconRefresh } from "@tabler/icons-svelte";
  import type { AppConfig, InputDevice } from "$lib/types";

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

<section class="space-y-6">
  <header>
    <h2 class="text-xl font-semibold tracking-tight">Audio</h2>
    <p class="mt-1 text-sm text-slate-400">
      Which microphone Oto records from, how the signal is conditioned before it reaches
      speech-to-text, and when a hands-free session ends on its own.
    </p>
  </header>

  <!-- Input device -->
  <div class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl">
    <div class="flex items-start justify-between gap-4">
      <div>
        <h3 class="text-sm font-semibold tracking-tight text-slate-200">Input device</h3>
        <p class="mt-1 text-xs text-slate-500">
          Oto falls back to the system default if the selected device disappears.
        </p>
      </div>
      <button
        type="button"
        class="flex shrink-0 items-center gap-1.5 rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-medium text-slate-200 transition hover:bg-white/10 disabled:opacity-50"
        disabled={loadingDevices}
        onclick={() => void loadDevices()}
      >
        <IconRefresh aria-hidden="true" size={14} stroke={1.8} />
        {loadingDevices ? "Scanning…" : "Rescan"}
      </button>
    </div>

    <label class="block space-y-1.5">
      <span class="text-sm font-medium text-slate-300">Microphone</span>
      <select
        class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2.5 text-sm text-white outline-none transition focus:border-sky-400/50 focus:ring-2 focus:ring-sky-400/20"
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
      {#if deviceError}
        <span class="block text-xs text-amber-200/80">Could not list devices ({deviceError}).</span>
      {:else if missingDevice}
        <span class="block text-xs text-amber-200/80">
          This device is not currently connected. Oto will record from the system default until it
          comes back.
        </span>
      {/if}
    </label>

    <label class="block space-y-1.5">
      <span class="flex items-center justify-between text-sm font-medium text-slate-300">
        <span>Input gain</span>
        <span class="font-mono text-xs text-slate-400">{config.audio.input_gain.toFixed(2)}×</span>
      </span>
      <input
        type="range"
        min="0.25"
        max="4"
        step="0.05"
        class="w-full accent-sky-400"
        bind:value={config.audio.input_gain}
      />
      <span class="block text-xs text-slate-500">
        Raise for a quiet microphone. Loud samples are clipped rather than wrapped, so overshooting
        distorts instead of crackling.
      </span>
    </label>

    <label
      class="flex cursor-pointer items-center justify-between gap-4 rounded-xl border border-white/10 bg-slate-900/40 px-4 py-3 transition hover:border-white/20"
    >
      <span>
        <span class="block text-sm font-medium text-slate-200">Noise gate</span>
        <span class="block text-xs text-slate-500">
          Attenuate audio that sits at the measured room-noise level. Helps with fans and keyboards;
          the threshold adapts to your room automatically.
        </span>
      </span>
      <input
        type="checkbox"
        class="h-4 w-4 shrink-0 rounded border-white/20 bg-slate-900 text-sky-500 focus:ring-sky-400/30"
        bind:checked={config.audio.noise_gate}
      />
    </label>
  </div>

  <!-- Activation -->
  <div class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl">
    <div>
      <h3 class="text-sm font-semibold tracking-tight text-slate-200">Activation</h3>
      <p class="mt-1 text-xs text-slate-500">How the hotkey starts and stops a dictation.</p>
    </div>

    <div class="space-y-2">
      {#each [
        {
          value: "hold",
          title: "Hold to talk",
          detail: "Press and hold while speaking, release to transcribe. The 0.1.0 behaviour.",
        },
        {
          value: "toggle",
          title: "Toggle",
          detail: "Press once to start, press again to stop. Hands-free while you speak.",
        },
        {
          value: "hybrid",
          title: "Hybrid",
          detail: "A quick tap toggles; holding the key past the tap threshold behaves like push-to-talk.",
        },
      ] as const as option (option.value)}
        <label
          class="flex cursor-pointer items-start gap-3 rounded-xl border px-4 py-3 transition"
          class:border-sky-400={config.activation_mode === option.value}
          class:bg-sky-400={config.activation_mode === option.value}
          class:bg-opacity-10={config.activation_mode === option.value}
          class:border-white={config.activation_mode !== option.value}
          class:border-opacity-10={config.activation_mode !== option.value}
        >
          <input
            type="radio"
            name="activation-mode"
            value={option.value}
            class="mt-0.5 h-4 w-4 shrink-0 border-white/20 bg-slate-900 text-sky-500 focus:ring-sky-400/30"
            bind:group={config.activation_mode}
          />
          <span class="min-w-0">
            <span class="block text-sm font-medium text-slate-200">{option.title}</span>
            <span class="block text-xs text-slate-500">{option.detail}</span>
          </span>
        </label>
      {/each}
    </div>

    {#if config.activation_mode === "hybrid"}
      <label class="block space-y-1.5">
        <span class="flex items-center justify-between text-sm font-medium text-slate-300">
          <span>Tap threshold</span>
          <span class="font-mono text-xs text-slate-400">{config.hybrid_tap_threshold_ms} ms</span>
        </span>
        <input
          type="range"
          min="120"
          max="1000"
          step="10"
          class="w-full accent-sky-400"
          bind:value={config.hybrid_tap_threshold_ms}
        />
        <span class="block text-xs text-slate-500">
          Releases faster than this count as a tap and leave the session running.
        </span>
      </label>
    {/if}
  </div>

  <!-- Auto-stop -->
  <div class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl">
    <div>
      <h3 class="text-sm font-semibold tracking-tight text-slate-200">Silence detection</h3>
      <p class="mt-1 text-xs text-slate-500">
        Ends a hands-free session once you stop speaking. Never applies while the hotkey is held —
        a held key always ends on its own release.
      </p>
    </div>

    <label
      class="flex cursor-pointer items-center justify-between gap-4 rounded-xl border border-white/10 bg-slate-900/40 px-4 py-3 transition hover:border-white/20"
    >
      <span>
        <span class="block text-sm font-medium text-slate-200">Stop after silence</span>
        <span class="block text-xs text-slate-500">
          Applies to Toggle and to hybrid taps.
        </span>
      </span>
      <input
        type="checkbox"
        class="h-4 w-4 shrink-0 rounded border-white/20 bg-slate-900 text-sky-500 focus:ring-sky-400/30"
        bind:checked={config.vad.auto_stop}
      />
    </label>

    {#if config.vad.auto_stop}
      <label class="block space-y-1.5">
        <span class="flex items-center justify-between text-sm font-medium text-slate-300">
          <span>Trailing silence</span>
          <span class="font-mono text-xs text-slate-400">
            {(config.vad.silence_ms / 1000).toFixed(1)} s
          </span>
        </span>
        <input
          type="range"
          min="400"
          max="5000"
          step="100"
          class="w-full accent-sky-400"
          bind:value={config.vad.silence_ms}
        />
        <span class="block text-xs text-slate-500">
          Shorter feels snappier but can cut you off mid-thought.
        </span>
      </label>

      <label class="block space-y-1.5">
        <span class="flex items-center justify-between text-sm font-medium text-slate-300">
          <span>Minimum speech</span>
          <span class="font-mono text-xs text-slate-400">{config.vad.min_speech_ms} ms</span>
        </span>
        <input
          type="range"
          min="0"
          max="2000"
          step="50"
          class="w-full accent-sky-400"
          bind:value={config.vad.min_speech_ms}
        />
        <span class="block text-xs text-slate-500">
          Speech required before auto-stop can fire, so a cough or a slow start does not end the
          session.
        </span>
      </label>
    {/if}
  </div>

  <!-- Sound cues -->
  <div class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl">
    <div>
      <h3 class="text-sm font-semibold tracking-tight text-slate-200">Sound cues</h3>
      <p class="mt-1 text-xs text-slate-500">
        Short tones marking session transitions — useful when the overlay is hidden or you are
        looking at another window.
      </p>
    </div>

    <label
      class="flex cursor-pointer items-center justify-between gap-4 rounded-xl border border-white/10 bg-slate-900/40 px-4 py-3 transition hover:border-white/20"
    >
      <span>
        <span class="block text-sm font-medium text-slate-200">Play sound cues</span>
        <span class="block text-xs text-slate-500">Rendered on the fly; no audio files involved.</span>
      </span>
      <input
        type="checkbox"
        class="h-4 w-4 shrink-0 rounded border-white/20 bg-slate-900 text-sky-500 focus:ring-sky-400/30"
        bind:checked={config.sounds.enabled}
      />
    </label>

    {#if config.sounds.enabled}
      <label class="block space-y-1.5">
        <span class="flex items-center justify-between text-sm font-medium text-slate-300">
          <span>Volume</span>
          <span class="font-mono text-xs text-slate-400">
            {Math.round(config.sounds.volume * 100)}%
          </span>
        </span>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          class="w-full accent-sky-400"
          bind:value={config.sounds.volume}
        />
      </label>

      <div class="space-y-2">
        {#each cues as cue (cue.id)}
          <div
            class="flex items-center justify-between gap-3 rounded-xl border border-white/10 bg-slate-900/40 px-4 py-2.5"
          >
            <label class="flex flex-1 cursor-pointer items-center gap-3">
              <input
                type="checkbox"
                class="h-4 w-4 shrink-0 rounded border-white/20 bg-slate-900 text-sky-500 focus:ring-sky-400/30"
                bind:checked={config.sounds[cue.key]}
              />
              <span class="text-sm text-slate-200">{cue.label}</span>
            </label>
            <button
              type="button"
              class="flex shrink-0 items-center gap-1.5 rounded-lg border border-white/10 bg-white/5 px-2.5 py-1 text-xs font-medium text-slate-300 transition hover:bg-white/10"
              onclick={() => void previewCue(cue.id)}
            >
              <IconPlayerPlay aria-hidden="true" size={13} stroke={1.8} />
              Preview
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <div
    class="flex items-start gap-3 rounded-2xl border border-white/10 bg-white/[0.02] px-5 py-4 text-xs leading-relaxed text-slate-400"
  >
    <span class="mt-0.5 shrink-0 text-slate-500">
      <IconMicrophone aria-hidden="true" size={18} stroke={1.7} />
    </span>
    <p>
      Use <strong class="text-slate-300">Permissions → Test microphone</strong> after changing these
      settings. The test records through the same device, gain, and gate that dictation uses, so a
      passing test means dictation will hear you too.
    </p>
  </div>
</section>
