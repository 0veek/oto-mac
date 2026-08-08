<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    IconAlertTriangle,
    IconArrowLeft,
    IconArrowRight,
    IconCheck,
    IconChevronDown,
  } from "@tabler/icons-svelte";
  import type { AppConfig, InputDevice, ProviderPreset } from "$lib/types";
  import Meter from "./Meter.svelte";

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

  // The Linux build asks the desktop whether a global shortcut can bind at all.
  // On macOS the equivalent question is whether Accessibility has been granted,
  // because that one permission gates both the shortcut and typing into apps.
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
      result = { ok: true, message: "Key saved to your Keychain." };
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

  const chordKeys = $derived(
    config.hotkey
      .split("+")
      .map((part) => part.trim())
      .filter(Boolean),
  );

  onMount(() => {
    void refreshAccessibility();
    invoke<InputDevice[]>("list_audio_inputs")
      .then((value) => (devices = value))
      .catch(() => (devices = []));
    void checkKey();
  });
</script>

<!-- The settings window has no title bar, so onboarding supplies the strip the
     traffic lights sit in and makes it the handle for moving the window. -->

<div class="setup">
  <!-- Setup genuinely is a sequence, so it is numbered, and the progress ladder
       reuses the meter language from the rest of the app. -->
  <div class="setup__progress">
    <div class="setup__count">
      <span class="plate-micro">
        Step {String(step + 1).padStart(2, "0")} of {String(STEPS.length).padStart(2, "0")}
      </span>
    </div>
    <ol class="ladder" aria-label="Setup progress">
      {#each STEPS as label, index (label)}
        <li
          class="ladder__rung"
          data-done={index < step}
          data-now={index === step}
          aria-current={index === step ? "step" : undefined}
        >
          <span class="ladder__name">{label}</span>
        </li>
      {/each}
    </ol>
  </div>

  <div class="setup__panel">
    {#if step === 0}
      <h1 class="setup__title">Welcome to Oto</h1>
      <p class="setup__lead">
        Hold a shortcut, speak, release. Oto writes down what you said, tidies it if you want, and
        types it into whatever you were using. About a minute to set up.
      </p>

      {#if accessibility && !accessibility.trusted}
        <div class="note note--bad setup__note">
          <IconAlertTriangle aria-hidden="true" size={16} stroke={1.8} />
          <span>
            <strong>Accessibility permission has not been granted.</strong>
            macOS wants it before Oto can register a global shortcut or type into other
            applications. Turn on <span class="readout-tight">{accessibility.display_name}</span>
            under System Settings → Privacy &amp; Security → Accessibility. You can still dictate
            from the menu bar, and setup will carry on either way.
            <span class="btn-row setup__grant">
              <button
                type="button"
                class="btn btn--small"
                disabled={busy !== null}
                onclick={() => void grantAccessibility()}
              >
                Open Accessibility settings
              </button>
              <button
                type="button"
                class="btn btn--quiet btn--small"
                onclick={() => void refreshAccessibility()}
              >
                Re-check
              </button>
            </span>
          </span>
        </div>
      {:else if accessibility}
        <p class="note note--ok setup__note">
          Accessibility is granted for
          <span class="readout-tight">{accessibility.display_name}</span>. Your shortcut will bind
          and Oto can type into other applications.
        </p>
      {/if}

      <label class="row row--switch row--flush setup__row">
        <span class="row__copy">
          <strong>Play sound cues</strong>
          <span>A short tone when recording starts and stops. Useful while the shortcut is new.</span>
        </span>
        <input type="checkbox" bind:checked={config.sounds.enabled} />
      </label>

    {:else if step === 1}
      <h1 class="setup__title">Choose a microphone</h1>
      <p class="setup__lead">
        Oto follows your system default unless you name something specific here.
      </p>

      <label class="field setup__field">
        <span class="plate-micro field__label">Input device</span>
        <div class="select-wrap">
          <select
            value={config.audio.input_device ?? ""}
            onchange={(event) => (config.audio.input_device = event.currentTarget.value || null)}
          >
            <option value="">System default</option>
            {#each devices as device (device.name)}
              <option value={device.name}>
                {device.name}{device.is_default ? " — default" : ""}
              </option>
            {/each}
          </select>
          <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
        </div>
      </label>

      <div class="setup__bench">
        <div class="setup__bench-head">
          <span class="plate-micro">Live input</span>
        </div>
        <Meter segments={32} variant="tall" />
      </div>

      <button
        type="button"
        class="btn setup__action"
        disabled={busy !== null}
        onclick={() => void run("test_microphone", "Microphone test")}
      >
        {busy === "test_microphone" ? "Listening for two seconds…" : "Test the microphone"}
      </button>
      <p class="setup__hint">
        Speak while the test runs. If the ladder above stays dark, choose a different device — and
        if macOS has not asked for the microphone yet, the first test is what prompts it.
      </p>

    {:else if step === 2}
      <h1 class="setup__title">Connect a provider</h1>
      <p class="setup__lead">
        Oto needs a service to turn speech into text. Your key goes to the macOS Keychain, never
        into a configuration file.
      </p>

      <div class="choice-list setup__choices">
        {#each PRESETS as preset (preset.value)}
          <button
            type="button"
            class="choice"
            data-active={config.provider_preset === preset.value}
            onclick={() => choosePreset(preset)}
          >
            <span class="choice__copy">
              <strong>{preset.name}</strong>
              <span>{preset.detail}</span>
            </span>
          </button>
        {/each}
      </div>

      <div class="btn-row setup__key">
        <input
          type="password"
          class="field-data setup__key-input"
          autocomplete="off"
          aria-label="API key"
          placeholder={keySaved ? "Enter a new key to replace the saved one" : "Paste your API key"}
          bind:value={apiKey}
        />
        <button
          type="button"
          class="btn btn--primary"
          disabled={!apiKey.trim() || busy !== null}
          onclick={() => void saveKey()}
        >
          Save
        </button>
      </div>

      {#if keySaved}
        <p class="setup__hint status-ok">
          <IconCheck aria-hidden="true" size={14} stroke={2.2} />
          Stored in the Keychain.
        </p>
      {/if}

      <p class="setup__hint">
        Would rather stay offline? Skip this and pick <strong>Models → On this machine</strong>
        later. Oto then never sends your audio anywhere.
      </p>

    {:else if step === 3}
      <h1 class="setup__title">Pick your shortcut</h1>
      <p class="setup__lead">
        Steer clear of chords macOS or your apps already own.
        <span class="readout-tight">Ctrl+Shift+…</span> is usually free; ⌘ chords rarely are.
      </p>

      <label class="field setup__field">
        <span class="plate-micro field__label">Dictation chord</span>
        <input type="text" class="field-data" spellcheck="false" autocomplete="off" bind:value={config.hotkey} />
        {#if chordKeys.length}
          <span class="keys" aria-hidden="true">
            {#each chordKeys as key, index (index)}
              {#if index > 0}<span class="keys__join">+</span>{/if}
              <kbd class="key">{key}</kbd>
            {/each}
          </span>
        {/if}
      </label>

      <div class="setup__field" role="radiogroup" aria-label="How it activates">
        <span class="plate-micro field__label">How it activates</span>
        <div class="choice-list">
          {#each [
            { value: "hold", title: "Hold to talk", detail: "Hold while speaking, release to transcribe." },
            { value: "toggle", title: "Toggle", detail: "Press once to start, once to stop." },
            { value: "hybrid", title: "Hybrid", detail: "Tap for hands free, hold for push-to-talk." },
          ] as const as option (option.value)}
            <label class="choice" data-active={config.activation_mode === option.value}>
              <input
                type="radio"
                name="onboarding-activation"
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

    {:else}
      <h1 class="setup__title">Try it out</h1>
      <p class="setup__lead">
        These run the real thing, in order. If all three pass, you are set.
      </p>

      <div class="checks">
        {#each [
          {
            command: "test_microphone",
            label: "Microphone test",
            title: "Record two seconds",
            detail: "Checks that Oto can hear you.",
          },
          {
            command: "test_transcription",
            label: "Transcription test",
            title: "Write down that recording",
            detail: "Checks your provider and your key.",
          },
          {
            command: "test_injection",
            label: "Insertion test",
            title: "Type into another window",
            detail: "Click into a text field in another app within a couple of seconds of starting this.",
          },
        ] as check, index (check.command)}
          <button
            type="button"
            class="check"
            disabled={busy !== null}
            onclick={() => void run(check.command, check.label)}
          >
            <span class="plate-micro check__index">{String(index + 1).padStart(2, "0")}</span>
            <span class="check__copy">
              <strong>{check.title}</strong>
              <span>{check.detail}</span>
            </span>
            <span class="check__state">{busy === check.command ? "Running…" : "Run"}</span>
          </button>
        {/each}
      </div>
    {/if}

    {#if result}
      <p
        aria-live="polite"
        class="note setup__note"
        class:note--ok={result.ok}
        class:note--bad={!result.ok}
      >
        {result.message}
      </p>
    {/if}
  </div>

  <div class="setup__foot">
    <button type="button" class="btn-link" onclick={() => void skip()}>Skip setup</button>

    <div class="btn-row">
      {#if step > 0}
        <button
          type="button"
          class="btn"
          onclick={() => {
            step -= 1;
            result = null;
          }}
        >
          <IconArrowLeft aria-hidden="true" size={15} stroke={1.8} />
          Back
        </button>
      {/if}
      {#if step < STEPS.length - 1}
        <button
          type="button"
          class="btn btn--primary"
          disabled={!canAdvance}
          onclick={() => {
            step += 1;
            result = null;
          }}
        >
          Next
          <IconArrowRight aria-hidden="true" size={15} stroke={1.8} />
        </button>
      {:else}
        <button type="button" class="btn btn--primary" disabled={busy !== null} onclick={() => void finish()}>
          Finish
        </button>
      {/if}
    </div>
  </div>

  {#if step === 2 && !canAdvance}
    <p class="setup__hint setup__hint--end">
      Save a key to carry on, or skip setup and choose a local model later.
    </p>
  {/if}
</div>

<style>
  .setup {
    display: flex;
    max-width: 44rem;
    /* The shell already reserves the traffic-light strip above us, so the column
       takes what is left of the viewport instead of forcing a scrollbar. */
    min-height: calc(100dvh - var(--mac-titlebar));
    flex-direction: column;
    justify-content: center;
    margin-inline: auto;
    padding: var(--space-xl) var(--space-md);
  }

  .setup__progress {
    display: grid;
    gap: 0.5rem;
    margin-block-end: var(--space-lg);
  }

  .setup__count {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-sm);
    color: var(--faint);
  }

  .ladder {
    display: grid;
    grid-auto-columns: minmax(0, 1fr);
    grid-auto-flow: column;
    gap: 3px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .ladder__rung {
    display: grid;
    gap: 0.375rem;
  }

  /* The rung itself is the bar; the label sits under it on wider screens. */
  .ladder__rung::before {
    height: 3px;
    border-radius: 1px;
    background: var(--etch);
    content: "";
    transition: background-color var(--dur-throw) var(--ease-lamp);
  }

  .ladder__rung[data-done="true"]::before {
    background: var(--lamp-deep);
  }

  .ladder__rung[data-now="true"]::before {
    background: var(--lamp);
  }

  .ladder__name {
    display: none;
    overflow: hidden;
    color: var(--faint);
    font-size: var(--text-micro);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ladder__rung[data-now="true"] .ladder__name {
    color: var(--ink-2);
  }

  .setup__panel {
    padding: var(--space-lg);
    border: var(--rule) solid var(--etch);
    border-radius: var(--radius-panel);
    background: var(--panel);
  }

  .setup__title {
    color: var(--ink);
    font-family: var(--font-plate);
    font-stretch: var(--plate-width);
    font-size: var(--text-lg);
    font-weight: 650;
    line-height: 1.15;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .setup__lead {
    max-width: 58ch;
    margin-block-start: 0.625rem;
    color: var(--muted);
    font-size: var(--text-base);
    line-height: 1.55;
  }

  .setup__note {
    margin-block-start: var(--space-md);
  }

  /* The two Accessibility controls hang under the warning copy, not beside it. */
  .setup__grant {
    margin-block-start: 0.5rem;
  }

  .setup__row,
  .setup__field,
  .setup__choices,
  .setup__key,
  .setup__bench {
    margin-block-start: var(--space-md);
  }

  .setup__action {
    margin-block-start: var(--space-md);
  }

  .setup__bench {
    display: grid;
    gap: 0.5rem;
    padding: 0.75rem 0.875rem 0.875rem;
    border: var(--rule) solid var(--etch);
    border-radius: var(--radius-panel);
    background: var(--well);
  }

  .setup__bench-head {
    color: var(--faint);
  }

  .setup__key {
    flex-wrap: nowrap;
  }

  .setup__key-input {
    min-width: 0;
    flex: 1;
  }

  .setup__hint {
    display: flex;
    align-items: center;
    gap: 0.375rem;
    max-width: 62ch;
    margin-block-start: 0.5rem;
    color: var(--muted);
    font-size: var(--text-xs);
    line-height: 1.55;
  }

  .setup__hint--end {
    justify-content: flex-end;
    margin-block-start: var(--space-sm);
  }

  .setup__hint strong {
    color: var(--ink-2);
    font-weight: 560;
  }

  .checks {
    display: grid;
    gap: 2px;
    margin-block-start: var(--space-md);
  }

  .check {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.6875rem 0.75rem;
    border: var(--rule) solid var(--etch);
    border-radius: var(--radius-control);
    background: var(--chassis);
    text-align: start;
    transition: border-color var(--dur-tick) var(--ease-mech);
  }

  .check:not(:disabled):hover {
    border-color: var(--etch-strong);
  }

  .check:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .check__index {
    flex: 0 0 auto;
    color: var(--faint);
  }

  .check__copy {
    display: grid;
    gap: 0.125rem;
    min-width: 0;
    flex: 1;
  }

  .check__copy strong {
    color: var(--ink-2);
    font-size: var(--text-sm);
    font-weight: 560;
  }

  .check__copy span {
    color: var(--muted);
    font-size: var(--text-xs);
    line-height: 1.5;
  }

  .check__state {
    flex: 0 0 auto;
    color: var(--lamp-text);
    font-size: var(--text-xs);
    font-weight: 540;
  }

  .setup__foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-md);
    margin-block-start: var(--space-md);
  }

  @media (min-width: 34rem) {
    .ladder__name {
      display: block;
    }
  }
</style>
