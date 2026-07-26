<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    IconAlertTriangle,
    IconArrowLeft,
    IconArrowRight,
    IconCheck,
  } from "@tabler/icons-svelte";
  import type { AppConfig, InputDevice, ProviderPreset } from "$lib/types";

  /** Subset of `get_accessibility_status` this wizard reads. */
  interface AccessibilityStatus {
    trusted: boolean;
    bundled: boolean;
    display_name: string;
    executable_path: string;
    bundle_id: string;
    guidance: string;
  }

  let {
    config = $bindable(),
    ondone,
  }: {
    config: AppConfig;
    ondone: () => void;
  } = $props();

  const STEPS = ["Welcome", "Microphone", "Provider", "Shortcut", "Try it"] as const;
  let step = $state(0);

  let accessibility = $state<AccessibilityStatus | null>(null);
  let devices = $state<InputDevice[]>([]);
  let apiKey = $state("");
  let keySaved = $state(false);
  let busy = $state<string | null>(null);
  let result = $state<{ ok: boolean; message: string } | null>(null);

  const PRESETS: { value: ProviderPreset; name: string; detail: string; base: string; stt: string; polish: string }[] = [
    {
      value: "deepgram",
      name: "Deepgram",
      detail: "Fastest. The only provider that supports live streaming transcription.",
      base: "https://api.deepgram.com",
      stt: "nova-3",
      polish: "",
    },
    {
      value: "groq",
      name: "Groq",
      detail: "Very fast Whisper, and an LLM for cleanup on the same key.",
      base: "https://api.groq.com/openai/v1",
      stt: "whisper-large-v3",
      polish: "llama-3.1-8b-instant",
    },
    {
      value: "open_ai",
      name: "OpenAI",
      detail: "Whisper plus GPT for cleanup.",
      base: "https://api.openai.com/v1",
      stt: "whisper-1",
      polish: "gpt-4o-mini",
    },
  ];

  function choosePreset(preset: (typeof PRESETS)[number]) {
    config.provider_preset = preset.value;
    config.base_url = preset.base;
    config.stt_model = preset.stt;
    // Deepgram has no chat endpoint, so cleanup cannot run on its key.
    if (preset.polish) {
      config.polish_model = preset.polish;
      config.polish_enabled = true;
    } else {
      config.polish_enabled = false;
    }
    keySaved = false;
    void checkKey();
  }

  async function refreshAccessibility() {
    try {
      accessibility = await invoke<AccessibilityStatus>("get_accessibility_status");
    } catch {
      accessibility = null;
    }
  }

  async function grantAccessibility() {
    busy = "accessibility";
    result = null;
    try {
      accessibility = await invoke<AccessibilityStatus>("request_accessibility");
      if (!accessibility.trusted) {
        // The prompt only opens the pane; the toggle is the user's to flip.
        await invoke("open_accessibility_settings_cmd");
      }
    } catch (error) {
      result = { ok: false, message: `Could not open Accessibility settings: ${String(error)}` };
    } finally {
      busy = null;
    }
  }

  async function checkKey() {
    try {
      keySaved = await invoke<boolean>("api_key_present", { preset: config.provider_preset });
    } catch {
      keySaved = false;
    }
  }

  async function saveKey() {
    if (!apiKey.trim()) return;
    busy = "key";
    result = null;
    try {
      await invoke("set_api_key", { preset: config.provider_preset, key: apiKey.trim() });
      apiKey = "";
      keySaved = true;
      result = { ok: true, message: "Key saved to your system keyring." };
    } catch (error) {
      result = { ok: false, message: `Could not save the key: ${String(error)}` };
    } finally {
      busy = null;
    }
  }

  async function run(command: string, label: string) {
    busy = command;
    result = null;
    try {
      // Persist first so the test exercises the settings just chosen.
      await invoke("set_config", { cfg: config });
      const message = await invoke<string | void>(command);
      result = { ok: true, message: message ? String(message) : `${label} passed.` };
    } catch (error) {
      result = { ok: false, message: `${label} failed: ${String(error)}` };
    } finally {
      busy = null;
    }
  }

  async function finish() {
    busy = "finish";
    try {
      config.onboarding_complete = true;
      await invoke("set_config", { cfg: config });
      ondone();
    } catch (error) {
      result = { ok: false, message: `Could not save: ${String(error)}` };
      busy = null;
    }
  }

  async function skip() {
    config.onboarding_complete = true;
    try {
      await invoke("set_config", { cfg: config });
    } catch {
      // Skipping should never be the thing that fails.
    }
    ondone();
  }

  let canAdvance = $derived.by(() => {
    // Only the provider step has a hard requirement, and a local endpoint or
    // local Whisper needs no key at all.
    if (step !== 2) return true;
    return keySaved || config.stt_backend === "local_whisper";
  });

  onMount(() => {
    void refreshAccessibility();
    invoke<InputDevice[]>("list_audio_inputs")
      .then((value) => (devices = value))
      .catch(() => (devices = []));
    void checkKey();
  });
</script>

<div class="mx-auto flex min-h-screen max-w-2xl flex-col justify-center px-6 py-12">
  <!-- Progress -->
  <ol class="mb-8 flex items-center gap-2" aria-label="Setup progress">
    {#each STEPS as label, index (label)}
      <li class="flex flex-1 items-center gap-2">
        <span
          class="flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-xs font-medium transition"
          class:bg-sky-500={index <= step}
          class:text-white={index <= step}
          class:bg-white={index > step}
          class:bg-opacity-10={index > step}
          class:text-slate-500={index > step}
        >
          {#if index < step}
            <IconCheck aria-hidden="true" size={14} stroke={2.4} />
          {:else}
            {index + 1}
          {/if}
        </span>
        <span class="hidden truncate text-xs sm:block" class:text-slate-200={index === step} class:text-slate-600={index !== step}>
          {label}
        </span>
      </li>
    {/each}
  </ol>

  <div class="rounded-3xl border border-white/10 bg-white/[0.04] p-8 shadow-2xl backdrop-blur-xl">
    {#if step === 0}
      <h1 class="text-2xl font-semibold tracking-tight text-slate-50">Welcome to Oto</h1>
      <p class="mt-3 text-sm leading-relaxed text-slate-400">
        Hold a shortcut, speak, release. Oto transcribes what you said, optionally cleans it up,
        and types it into whatever you were using. This takes about a minute to set up.
      </p>

      {#if accessibility && !accessibility.trusted}
        <div class="mt-6 flex items-start gap-3 rounded-2xl border border-amber-400/30 bg-amber-400/10 px-5 py-4 text-sm text-amber-50">
          <span class="mt-0.5 shrink-0 text-amber-300">
            <IconAlertTriangle aria-hidden="true" size={18} stroke={1.8} />
          </span>
          <div class="min-w-0">
            <p class="font-medium text-amber-100">Accessibility permission is not granted yet</p>
            <p class="mt-1 text-amber-50/90">
              macOS needs it before Oto can register a global shortcut or type into other
              applications. Enable “{accessibility.display_name}” under System Settings → Privacy &
              Security → Accessibility, then reopen Oto.
            </p>
            <div class="mt-3 flex flex-wrap items-center gap-2">
              <button
                type="button"
                class="rounded-lg bg-white/10 px-3 py-1.5 text-xs font-medium text-amber-50 transition hover:bg-white/15 disabled:opacity-50"
                disabled={busy !== null}
                onclick={() => void grantAccessibility()}
              >
                Open Accessibility settings
              </button>
              <button
                type="button"
                class="rounded-lg px-2 py-1.5 text-xs text-amber-100/70 transition hover:text-amber-50"
                onclick={() => void refreshAccessibility()}
              >
                Re-check
              </button>
            </div>
            <p class="mt-2 text-amber-50/70">
              You can still dictate from the menu bar. Setup will continue either way.
            </p>
          </div>
        </div>
      {:else if accessibility}
        <p class="mt-6 rounded-2xl border border-emerald-400/20 bg-emerald-400/5 px-5 py-3 text-xs text-emerald-100/90">
          Accessibility is granted for
          <code class="rounded bg-white/5 px-1 font-mono">{accessibility.display_name}</code>.
          Global shortcuts and typing into other applications will work.
        </p>
      {/if}

      <label class="mt-6 flex cursor-pointer items-center justify-between gap-4 rounded-2xl border border-white/10 bg-slate-900/40 px-5 py-4">
        <span>
          <span class="block text-sm font-medium text-slate-200">Play sound cues</span>
          <span class="block text-xs text-slate-500">
            A short tone when recording starts and stops. Handy while you learn the shortcut.
          </span>
        </span>
        <input type="checkbox" class="h-4 w-4 shrink-0 rounded border-white/20 bg-slate-900 text-sky-500" bind:checked={config.sounds.enabled} />
      </label>

    {:else if step === 1}
      <h1 class="text-2xl font-semibold tracking-tight text-slate-50">Choose a microphone</h1>
      <p class="mt-3 text-sm leading-relaxed text-slate-400">
        Oto follows your system default unless you pick something specific.
      </p>

      <label class="mt-6 block space-y-2">
        <span class="text-sm font-medium text-slate-300">Input device</span>
        <select
          class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2.5 text-sm text-white outline-none focus:border-sky-400/50"
          value={config.audio.input_device ?? ""}
          onchange={(event) => (config.audio.input_device = event.currentTarget.value || null)}
        >
          <option value="">System default</option>
          {#each devices as device (device.name)}
            <option value={device.name}>{device.name}{device.is_default ? " — default" : ""}</option>
          {/each}
        </select>
      </label>

      <button
        type="button"
        class="mt-5 rounded-xl bg-white/10 px-4 py-2.5 text-sm text-slate-100 transition hover:bg-white/15 disabled:opacity-50"
        disabled={busy !== null}
        onclick={() => void run("test_microphone", "Microphone test")}
      >
        {busy === "test_microphone" ? "Listening for two seconds…" : "Test microphone"}
      </button>
      <p class="mt-2 text-xs text-slate-500">
        Speak while the overlay shows a waveform. If the bars stay flat, pick a different device.
      </p>

    {:else if step === 2}
      <h1 class="text-2xl font-semibold tracking-tight text-slate-50">Connect a provider</h1>
      <p class="mt-3 text-sm leading-relaxed text-slate-400">
        Oto needs a speech-to-text service. Your key is stored in the system keyring, never in the
        config file.
      </p>

      <div class="mt-6 space-y-2">
        {#each PRESETS as preset (preset.value)}
          <button
            type="button"
            class="flex w-full items-start gap-3 rounded-2xl border px-5 py-4 text-left transition"
            class:border-sky-400={config.provider_preset === preset.value}
            class:bg-sky-400={config.provider_preset === preset.value}
            class:bg-opacity-10={config.provider_preset === preset.value}
            class:border-white={config.provider_preset !== preset.value}
            class:border-opacity-10={config.provider_preset !== preset.value}
            onclick={() => choosePreset(preset)}
          >
            <span class="min-w-0">
              <span class="block text-sm font-medium text-slate-100">{preset.name}</span>
              <span class="block text-xs text-slate-500">{preset.detail}</span>
            </span>
          </button>
        {/each}
      </div>

      <div class="mt-5 flex gap-2">
        <input
          type="password"
          autocomplete="off"
          placeholder={keySaved ? "A key is already saved — enter one to replace it" : "Paste your API key"}
          class="min-w-0 flex-1 rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2.5 text-sm text-white outline-none focus:border-sky-400/50"
          bind:value={apiKey}
        />
        <button
          type="button"
          class="shrink-0 rounded-xl bg-sky-500 px-4 py-2.5 text-sm font-medium text-white transition hover:bg-sky-400 disabled:opacity-50"
          disabled={!apiKey.trim() || busy !== null}
          onclick={() => void saveKey()}
        >
          Save
        </button>
      </div>
      {#if keySaved}
        <p class="mt-2 flex items-center gap-1.5 text-xs text-emerald-300">
          <IconCheck aria-hidden="true" size={14} stroke={2.2} /> Key stored in the keyring.
        </p>
      {/if}
      <p class="mt-3 text-xs leading-relaxed text-slate-500">
        Prefer to stay offline? Skip this and choose <strong class="text-slate-400">Models → Local
        Whisper</strong> later — Oto then never sends audio anywhere.
      </p>

    {:else if step === 3}
      <h1 class="text-2xl font-semibold tracking-tight text-slate-50">Pick your shortcut</h1>
      <p class="mt-3 text-sm leading-relaxed text-slate-400">
        Avoid chords macOS or your apps already use. <code class="rounded bg-white/5 px-1 font-mono">Ctrl+Shift+…</code>
        is usually free; ⌘-based chords rarely are.
      </p>

      <label class="mt-6 block space-y-2">
        <span class="text-sm font-medium text-slate-300">Dictation shortcut</span>
        <input
          type="text"
          spellcheck="false"
          autocomplete="off"
          class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2.5 font-mono text-sm text-white outline-none focus:border-sky-400/50"
          bind:value={config.hotkey}
        />
      </label>

      <fieldset class="mt-6 space-y-2">
        <legend class="text-sm font-medium text-slate-300">How it activates</legend>
        {#each [
          { value: "hold", title: "Hold to talk", detail: "Hold while speaking, release to transcribe." },
          { value: "toggle", title: "Toggle", detail: "Press once to start, once to stop." },
          { value: "hybrid", title: "Hybrid", detail: "Tap for hands-free, hold for push-to-talk." },
        ] as const as option (option.value)}
          <label class="flex cursor-pointer items-start gap-3 rounded-xl border border-white/10 bg-slate-900/40 px-4 py-3">
            <input
              type="radio"
              name="onboarding-activation"
              value={option.value}
              class="mt-0.5 h-4 w-4 shrink-0 border-white/20 bg-slate-900 text-sky-500"
              bind:group={config.activation_mode}
            />
            <span class="min-w-0">
              <span class="block text-sm text-slate-200">{option.title}</span>
              <span class="block text-xs text-slate-500">{option.detail}</span>
            </span>
          </label>
        {/each}
      </fieldset>

    {:else}
      <h1 class="text-2xl font-semibold tracking-tight text-slate-50">Try it out</h1>
      <p class="mt-3 text-sm leading-relaxed text-slate-400">
        These run the real pipeline. If both pass, you are ready.
      </p>

      <div class="mt-6 space-y-2">
        <button
          type="button"
          class="w-full rounded-xl bg-white/10 px-4 py-3 text-left text-sm text-slate-100 transition hover:bg-white/15 disabled:opacity-50"
          disabled={busy !== null}
          onclick={() => void run("test_microphone", "Microphone test")}
        >
          <span class="block font-medium">1 · Record two seconds</span>
          <span class="block text-xs text-slate-500">Checks that Oto can hear you.</span>
        </button>
        <button
          type="button"
          class="w-full rounded-xl bg-white/10 px-4 py-3 text-left text-sm text-slate-100 transition hover:bg-white/15 disabled:opacity-50"
          disabled={busy !== null}
          onclick={() => void run("test_transcription", "Transcription test")}
        >
          <span class="block font-medium">2 · Transcribe that recording</span>
          <span class="block text-xs text-slate-500">Checks your provider and key.</span>
        </button>
        <button
          type="button"
          class="w-full rounded-xl bg-white/10 px-4 py-3 text-left text-sm text-slate-100 transition hover:bg-white/15 disabled:opacity-50"
          disabled={busy !== null}
          onclick={() => void run("test_injection", "Insertion test")}
        >
          <span class="block font-medium">3 · Type into another window</span>
          <span class="block text-xs text-slate-500">
            Focus a text field in another app within a couple of seconds after pressing this.
          </span>
        </button>
      </div>
    {/if}

    {#if result}
      <p
        aria-live="polite"
        class="mt-5 rounded-xl px-4 py-3 text-sm leading-relaxed {result.ok
          ? 'border border-emerald-400/25 bg-emerald-400/5 text-emerald-100'
          : 'border border-rose-400/25 bg-rose-400/5 text-rose-100'}"
      >
        {result.message}
      </p>
    {/if}
  </div>

  <div class="mt-6 flex items-center justify-between gap-4">
    <button
      type="button"
      class="text-xs text-slate-500 transition hover:text-slate-300"
      onclick={() => void skip()}
    >
      Skip setup
    </button>

    <div class="flex items-center gap-2">
      {#if step > 0}
        <button
          type="button"
          class="flex items-center gap-1.5 rounded-xl bg-white/10 px-4 py-2.5 text-sm text-slate-100 transition hover:bg-white/15"
          onclick={() => {
            step -= 1;
            result = null;
          }}
        >
          <IconArrowLeft aria-hidden="true" size={16} stroke={1.8} />
          Back
        </button>
      {/if}
      {#if step < STEPS.length - 1}
        <button
          type="button"
          class="flex items-center gap-1.5 rounded-xl bg-sky-500 px-5 py-2.5 text-sm font-medium text-white transition hover:bg-sky-400 disabled:opacity-40"
          disabled={!canAdvance}
          onclick={() => {
            step += 1;
            result = null;
          }}
        >
          Next
          <IconArrowRight aria-hidden="true" size={16} stroke={1.8} />
        </button>
      {:else}
        <button
          type="button"
          class="rounded-xl bg-sky-500 px-5 py-2.5 text-sm font-medium text-white transition hover:bg-sky-400 disabled:opacity-50"
          disabled={busy !== null}
          onclick={() => void finish()}
        >
          Finish
        </button>
      {/if}
    </div>
  </div>

  {#if step === 2 && !canAdvance}
    <p class="mt-3 text-right text-xs text-slate-500">
      Save a key to continue, or skip setup and configure a local model later.
    </p>
  {/if}
</div>
