<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";

  let version = $state<string>("…");
  let versionError = $state<string | null>(null);

  onMount(async () => {
    try {
      version = await invoke<string>("get_app_version");
    } catch (e) {
      version = "0.1.0";
      versionError = String(e);
    }
  });
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">About</h2>
    <p class="section__lead">System-wide voice dictation for macOS.</p>
  </header>

  <!-- The equipment plate: what this is, and which build you are running. -->
  <div class="nameplate">
    <div class="nameplate__mark">
      <img src="/favicon.png" alt="" width="34" height="34" />
    </div>
    <div class="nameplate__copy">
      <span class="nameplate__name">Oto</span>
      <span class="readout nameplate__version">v{version}</span>
    </div>
    <span class="readout-tight nameplate__id">dev.oto.mac</span>
  </div>

  {#if versionError}
    <p class="row__hint">Could not read the version from Tauri ({versionError}).</p>
  {/if}

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Where your words go</span>
    </div>

    <div class="row row--stacked row--flush">
      <span class="row__label">In short</span>
      <p class="row__control about-body">
        Cloud transcription sends your audio to the provider you chose. Transcribing on this
        machine sends nothing anywhere. Cleanup and command mode send text — never audio — to your
        configured model. API keys live in the macOS Keychain, history stays on this disk, and sync
        only ever talks to the endpoint you enter yourself. There is no Oto server in the middle.
      </p>
    </div>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">On disk</span>
    </div>

    <div class="row">
      <span class="row__label">Settings</span>
      <p class="row__control row__hint">
        <span class="readout-tight">~/Library/Application Support/dev.Oto.oto/</span> — no secrets
        in it, and the exact path follows the app's own configuration directory.
      </p>
    </div>

    <div class="row row--flush">
      <span class="row__label">This build</span>
      <p class="row__control row__hint">
        The macOS port of Oto — the same pipeline as the Linux app, with Accessibility and Core
        Graphics doing the text insertion.
      </p>
    </div>
  </div>
</section>

<style>
  .nameplate {
    display: flex;
    align-items: center;
    gap: 0.875rem;
    padding: 0.875rem 1rem;
    border: var(--rule) solid var(--etch);
    border-radius: var(--radius-panel);
    background: var(--panel);
  }

  .nameplate__mark {
    display: grid;
    width: 2.75rem;
    height: 2.75rem;
    flex: 0 0 auto;
    place-items: center;
    border: var(--rule) solid var(--etch);
    border-radius: var(--radius-control);
    background: var(--well);
  }

  .nameplate__copy {
    display: grid;
    gap: 0.125rem;
    flex: 1;
    min-width: 0;
  }

  .nameplate__name {
    color: var(--ink);
    font-family: var(--font-plate);
    font-stretch: 122%;
    font-size: var(--text-md);
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
  }

  .nameplate__version {
    color: var(--lamp-text);
    font-size: var(--text-sm);
  }

  .nameplate__id {
    flex: 0 0 auto;
    color: var(--faint);
    font-size: var(--text-micro);
  }

  .about-body {
    max-width: 68ch;
    color: var(--muted);
    font-size: var(--text-sm);
    line-height: 1.65;
  }
</style>
