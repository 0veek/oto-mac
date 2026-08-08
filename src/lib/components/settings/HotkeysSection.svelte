<script lang="ts">
  import type { AppConfig } from "$lib/types";

  let {
    config = $bindable(),
  }: {
    config: AppConfig;
  } = $props();

  // A chord reads as keys, not as a string, so it is split for the cap preview.
  const chordKeys = $derived(
    config.hotkey
      .split("+")
      .map((part) => part.trim())
      .filter(Boolean),
  );
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Hotkeys</h2>
    <p class="section__lead">
      The chord that starts a dictation from anywhere. Hold it, speak, release. macOS
      registers the chord through the system global-shortcut API.
    </p>
  </header>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Shortcut</span>
    </div>

    <label class="row">
      <span class="row__label">Dictation chord</span>
      <span class="row__control">
        <input
          type="text"
          class="field-data"
          placeholder="Ctrl+Shift+Space"
          spellcheck="false"
          autocomplete="off"
          bind:value={config.hotkey}
        />
        {#if chordKeys.length}
          <span class="keys" aria-hidden="true">
            {#each chordKeys as key, index (index)}
              {#if index > 0}<span class="keys__join">+</span>{/if}
              <kbd class="key">{key}</kbd>
            {/each}
          </span>
        {/if}
        <span class="row__hint">
          Modifiers plus one key, joined by <span class="readout-tight">+</span>. Ctrl,
          Cmd/Super/Meta, Option/Alt and Shift are accepted, along with Space, Enter, Tab,
          Escape and a–z.
        </span>
      </span>
    </label>

    <div class="row row--stacked row--flush">
      <span class="row__label">Troubleshooting</span>
      <div class="row__control">
        <details class="disclosure">
          <summary>The overlay never appears</summary>
          <div class="disclosure__body">
            <ul>
              <li>Save after changing the chord — nothing binds until you do.</li>
              <li>
                Grant <strong>Accessibility</strong> — and Input Monitoring, if macOS asks for
                it — under System Settings → Privacy &amp; Security.
              </li>
              <li>
                Skip chords macOS already owns:
                <span class="readout-tight">Cmd+Space</span> for Spotlight and
                <span class="readout-tight">Cmd+Tab</span> for the app switcher.
              </li>
              <li>
                <span class="readout-tight">Ctrl+Shift+Space</span> and
                <span class="readout-tight">Cmd+Shift+D</span> are reliable choices.
              </li>
              <li>
                The menu bar icon's <strong>Start Listening</strong> works without any key grab.
              </li>
            </ul>
          </div>
        </details>
      </div>
    </div>
  </div>
</section>
