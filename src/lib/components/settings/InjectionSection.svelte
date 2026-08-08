<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { IconAlertTriangle, IconShieldCheck } from "@tabler/icons-svelte";
  import type { AppConfig, InjectionMode } from "$lib/types";

  let {
    config = $bindable(),
  }: {
    config: AppConfig;
  } = $props();

  let testBusy = $state(false);
  let testResult = $state<string | null>(null);
  let testError = $state<string | null>(null);

  type AxStatus = {
    trusted: boolean;
    bundled: boolean;
    displayName: string;
    executablePath: string;
    bundleId: string;
    guidance: string;
  };

  let ax = $state<AxStatus | null>(null);
  let axBusy = $state(false);
  let axError = $state<string | null>(null);

  const MODES: { value: InjectionMode; label: string; hint: string }[] = [
    {
      value: "auto",
      label: "Auto",
      hint: "Accessibility insert, then clipboard + ⌘V, then direct typing, then clipboard only.",
    },
    {
      value: "direct_type",
      label: "Direct type",
      hint: "Type character-by-character with synthetic key events (slower on long text).",
    },
    {
      value: "clipboard_paste",
      label: "Clipboard + paste",
      hint: "Always copy, then simulate ⌘V.",
    },
    {
      value: "clipboard_only",
      label: "Clipboard only",
      hint: "Copy text and prompt you to paste (⌘V).",
    },
  ];

  async function refreshAx() {
    try {
      ax = await invoke<AxStatus>("get_accessibility_status");
      axError = null;
    } catch (e) {
      axError = String(e);
    }
  }

  // Trust is granted outside the app and never announced, so it is polled.
  onMount(() => {
    void refreshAx();
    const id = setInterval(() => {
      void refreshAx();
    }, 2500);
    return () => clearInterval(id);
  });

  async function openAccessibility() {
    axBusy = true;
    axError = null;
    try {
      await invoke("open_accessibility_settings_cmd");
      await invoke("request_accessibility");
      await refreshAx();
    } catch (e) {
      axError = String(e);
    } finally {
      axBusy = false;
    }
  }

  async function revealInFinder() {
    axBusy = true;
    axError = null;
    try {
      await invoke("reveal_app_in_finder");
    } catch (e) {
      axError = String(e);
    } finally {
      axBusy = false;
    }
  }

  async function testInjection() {
    testBusy = true;
    testResult = null;
    testError = null;
    try {
      // Persist mode first so the command reads the selection.
      await invoke("set_config", { cfg: config });
      testResult = await invoke<string>("test_injection");
      await refreshAx();
    } catch (e) {
      testError = String(e);
      // A failure is usually revoked trust, so re-read it alongside the error.
      await refreshAx();
    } finally {
      testBusy = false;
    }
  }
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Insertion</h2>
    <p class="section__lead">
      How finished text reaches the window you were working in. Auto works almost
      everywhere; the rest are for when a particular application is stubborn.
    </p>
  </header>

  <div
    role="status"
    class="note"
    class:note--ok={ax?.trusted}
    class:note--warn={ax != null && !ax.trusted}
  >
    {#if ax?.trusted}
      <IconShieldCheck aria-hidden="true" size={16} stroke={1.7} />
    {:else}
      <IconAlertTriangle aria-hidden="true" size={16} stroke={1.8} />
    {/if}
    <div class="ax">
      <p>
        <strong>
          Accessibility {ax ? (ax.trusted ? "granted" : "not granted") : "— checking…"}
        </strong>
        {#if ax}
          <span class="readout-tight ax__name">
            listed as {ax.displayName}{ax.bundled ? "" : " (dev binary)"}
          </span>
        {/if}
      </p>

      {#if ax}
        <p class="ax__guidance">{ax.guidance}</p>

        {#if !ax.trusted}
          <ol>
            <li>Open <strong>Accessibility Settings</strong> below.</li>
            <li>Click the lock and enter your Mac password.</li>
            <li>Click <strong>+</strong> — Oto does not always appear on its own.</li>
            <li>
              {#if ax.bundled}
                Select <strong>{ax.displayName}.app</strong>, or use
                <strong>Reveal in Finder</strong> and drag it in.
              {:else}
                Press <kbd class="key">⌘⇧G</kbd> in the file picker and paste the path below, or
                use <strong>Reveal in Finder</strong>.
              {/if}
            </li>
            <li>Turn the toggle <strong>on</strong>, then quit and reopen Oto.</li>
          </ol>

          {#if ax.executablePath}
            <pre class="output">{ax.executablePath}</pre>
          {/if}
        {/if}
      {/if}

      <div class="btn-row">
        <button type="button" class="btn btn--small" disabled={axBusy} onclick={openAccessibility}>
          {axBusy ? "Opening…" : "Open Accessibility Settings"}
        </button>
        <button type="button" class="btn btn--small" disabled={axBusy} onclick={revealInFinder}>
          Reveal in Finder
        </button>
        <button type="button" class="btn btn--small" onclick={() => refreshAx()}>
          Refresh status
        </button>
      </div>

      {#if axError}
        <p role="alert" class="status-bad">{axError}</p>
      {/if}
    </div>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Method</span>
    </div>

    <div class="row row--stacked row--flush" role="radiogroup" aria-label="How to insert">
      <span class="row__label">How to insert</span>
      <div class="row__control choice-list">
        {#each MODES as mode (mode.value)}
          <label class="choice" data-active={config.injection_mode === mode.value}>
            <input
              type="radio"
              name="injection_mode"
              value={mode.value}
              checked={config.injection_mode === mode.value}
              onchange={() => {
                config.injection_mode = mode.value;
              }}
            />
            <span class="choice__copy">
              <strong>{mode.label}</strong>
              <span>{mode.hint}</span>
            </span>
          </label>
        {/each}
      </div>
    </div>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Test</span>
      <p class="rack__note">
        Start the test, then click into a text field in another application. Oto waits a moment and
        inserts <span class="readout-tight">Oto injection test</span> using the method above.
      </p>
    </div>

    <div class="row row--switch row--flush">
      <span class="row__copy">
        <strong>Insert test text</strong>
        <span>Uses the exact path a real dictation would take.</span>
      </span>
      <button type="button" class="btn" disabled={testBusy} onclick={testInjection}>
        {testBusy ? "Inserting…" : "Run test"}
      </button>
    </div>

    {#if testResult}
      <p aria-live="polite" class="note note--ok test-note">{testResult}</p>
    {/if}
    {#if testError}
      <p role="alert" class="note note--bad test-note">{testError}</p>
    {/if}
  </div>
</section>

<style>
  .ax {
    display: grid;
    gap: 0.5rem;
    min-width: 0;
  }

  .ax__name {
    color: var(--faint);
    font-size: var(--text-micro);
  }

  /* The backend writes its guidance as multiple lines. */
  .ax__guidance {
    white-space: pre-line;
  }

  .ax ol {
    display: grid;
    gap: 0.3125rem;
    margin: 0;
    padding-inline-start: 1.125rem;
  }

  .test-note {
    margin-block-start: var(--space-sm);
  }
</style>
