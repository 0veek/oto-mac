<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
  import { IconChevronDown } from "@tabler/icons-svelte";
  import type { AppConfig, IdleBehavior, ThemePreset } from "$lib/types";

  let {
    config = $bindable(),
  }: {
    config: AppConfig;
  } = $props();

  let previewBusy = $state(false);
  let micBusy = $state(false);
  let status = $state<string | null>(null);

  // Launch at login is not a config field on macOS: the login item lives in the
  // system's own registry, so the plugin is the single source of truth and this
  // control writes through immediately rather than waiting for Save.
  let launchAtLogin = $state(false);
  let launchBusy = $state(false);
  let launchError = $state<string | null>(null);
  let launchAvailable = $state(true);

  onMount(() => {
    void isEnabled()
      .then((value) => {
        launchAtLogin = value;
        launchAvailable = true;
      })
      .catch(() => {
        // Browser preview / missing plugin bindings
        launchAvailable = false;
      });
  });

  async function setLaunchAtLogin(next: boolean) {
    if (launchBusy || next === launchAtLogin) return;
    launchBusy = true;
    launchError = null;
    const previous = launchAtLogin;
    launchAtLogin = next;
    try {
      if (next) {
        await enable();
      } else {
        await disable();
      }
      launchAtLogin = await isEnabled();
    } catch (error) {
      launchAtLogin = previous;
      launchError = `Could not update launch at login: ${String(error)}`;
    } finally {
      launchBusy = false;
    }
  }

  const IDLE_OPTIONS: {
    value: IdleBehavior;
    label: string;
    hint: string;
  }[] = [
    {
      value: "hide",
      label: "Hide when idle",
      hint: "Overlay only appears during listening, processing, done, or error.",
    },
    {
      value: "minimal",
      label: "Minimal dormant pill",
      hint: "Keep a small dormant pill visible when idle so you can find Oto.",
    },
  ];

  const THEMES: { value: ThemePreset; label: string }[] = [
    { value: "system", label: "System" },
    { value: "midnight", label: "Midnight" },
    { value: "light", label: "Light" },
    { value: "high_contrast", label: "High contrast" },
  ];

  async function previewListening() {
    previewBusy = true;
    status = null;
    try {
      await invoke("debug_preview_listening");
      status = "Preview finished — the overlay should have shown Listening briefly.";
    } catch (e) {
      status = `Preview failed: ${String(e)}`;
    } finally {
      previewBusy = false;
    }
  }

  async function testMic() {
    micBusy = true;
    status = null;
    try {
      await invoke("test_microphone");
      status = "Mic test finished (~2s of levels on the overlay).";
    } catch (e) {
      status = `Mic test failed: ${String(e)}`;
    } finally {
      micBusy = false;
    }
  }
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Appearance</h2>
    <p class="section__lead">
      Startup, overlay idle behavior, and quick UI previews without a full
      dictation run.
    </p>
  </header>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Startup</span>
    </div>

    <label
      class="row row--switch row--flush launch"
      data-live={launchAvailable && !launchBusy}
    >
      <span class="row__copy">
        <strong>Launch at login</strong>
        <span>Start Oto in the menu bar when you sign in to macOS. Applies immediately.</span>
      </span>
      <input
        type="checkbox"
        checked={launchAtLogin}
        disabled={!launchAvailable || launchBusy}
        onchange={(event) => {
          void setLaunchAtLogin(event.currentTarget.checked);
        }}
      />
    </label>

    {#if launchError}
      <p class="note note--bad appearance-status" role="alert">{launchError}</p>
    {/if}
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Display</span>
    </div>

    <label class="row">
      <span class="row__label">Theme</span>
      <span class="row__control select-wrap">
        <select bind:value={config.theme}>
          {#each THEMES as theme (theme.value)}
            <option value={theme.value}>{theme.label}</option>
          {/each}
        </select>
        <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
      </span>
    </label>

    <label class="row">
      <span class="row__label">Text size</span>
      <span class="row__control">
        <span class="slider-head">
          <span class="row__hint">Scales everything in this window.</span>
          <span class="slider-value">{Math.round(config.font_scale * 100)}%</span>
        </span>
        <input type="range" min="0.85" max="1.25" step="0.05" bind:value={config.font_scale} />
      </span>
    </label>

    <label class="row row--switch row--flush">
      <span class="row__copy">
        <strong>Reduce motion</strong>
        <span>Stops the pulses and transitions that are not carrying information.</span>
      </span>
      <input type="checkbox" bind:checked={config.reduce_motion} />
    </label>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Overlay</span>
    </div>

    <div class="row row--stacked row--flush" role="radiogroup" aria-label="Between dictations">
      <span class="row__label">Between dictations</span>
      <div class="row__control choice-list">
        {#each IDLE_OPTIONS as opt (opt.value)}
          <label class="choice" data-active={config.idle_behavior === opt.value}>
            <input
              type="radio"
              name="idle_behavior"
              value={opt.value}
              checked={config.idle_behavior === opt.value}
              onchange={() => {
                config.idle_behavior = opt.value;
              }}
            />
            <span class="choice__copy">
              <strong>{opt.label}</strong>
              <span>{opt.hint}</span>
            </span>
          </label>
        {/each}
      </div>
    </div>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Check the overlay</span>
    </div>

    <div class="row row--switch">
      <span class="row__copy">
        <strong>Show a fake dictation</strong>
        <span>Drives the overlay with invented levels so you can see where it sits.</span>
      </span>
      <button type="button" class="btn" disabled={previewBusy} onclick={previewListening}>
        {previewBusy ? "Showing…" : "Show it"}
      </button>
    </div>

    <div class="row row--switch row--flush">
      <span class="row__copy">
        <strong>Test the microphone</strong>
        <span>Opens your input for about two seconds and sends real levels to the overlay.</span>
      </span>
      <button type="button" class="btn" disabled={micBusy} onclick={testMic}>
        {micBusy ? "Listening…" : "Listen"}
      </button>
    </div>

    {#if status}
      <p
        class="note appearance-status"
        class:note--bad={status.toLowerCase().includes("failed")}
      >
        {status}
      </p>
    {/if}
  </div>
</section>

<style>
  .appearance-status {
    margin-block-start: var(--space-sm);
  }

  /* In-flight or unavailable (browser preview has no plugin bindings): the copy
     drops to the faint ink and the row stops reading as a live control. The
     switch itself is disabled, which the shell already dims. */
  .launch[data-live="false"] {
    cursor: default;
  }

  .launch[data-live="false"] strong,
  .launch[data-live="false"] .row__copy span {
    color: var(--faint);
  }
</style>
