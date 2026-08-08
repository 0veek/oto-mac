<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import {
    IconChevronDown,
    IconChevronUp,
    IconPlus,
    IconTargetArrow,
    IconTrash,
  } from "@tabler/icons-svelte";
  import type { AppConfig, Mode } from "$lib/types";

  let {
    config = $bindable(),
  }: {
    config: AppConfig;
  } = $props();

  let expanded = $state<string | null>(null);
  let focusProbe = $state<string | null>(null);
  let probing = $state(false);

  function newMode(): Mode {
    return {
      // crypto.randomUUID is available in the WKWebView Tauri 2 uses on macOS.
      id: crypto.randomUUID(),
      name: "New mode",
      enabled: true,
      match: { app_classes: [], title_contains: "" },
      hotkey: "",
      stt_backend: null,
      provider_preset: null,
      stt_model: null,
      language: null,
      polish_enabled: null,
      polish_model: null,
      active_style_id: null,
      tone_hint: null,
      dictionary: [],
      injection_mode: null,
      context_level: null,
    };
  }

  function addMode() {
    const mode = newMode();
    config.modes = [...config.modes, mode];
    expanded = mode.id;
  }

  function removeMode(id: string) {
    config.modes = config.modes.filter((mode) => mode.id !== id);
    if (expanded === id) expanded = null;
  }

  // Order is meaningful: the first matching mode wins, so overlapping rules
  // resolve from this list rather than from something invisible.
  function move(index: number, delta: number) {
    const target = index + delta;
    if (target < 0 || target >= config.modes.length) return;
    const next = [...config.modes];
    [next[index], next[target]] = [next[target], next[index]];
    config.modes = next;
  }

  // `app_classes` is the wire name shared with Linux; on macOS the entries are
  // application names and bundle ids, which is what the copy below says.
  function setClasses(mode: Mode, raw: string) {
    mode.match.app_classes = raw
      .split(",")
      .map((entry) => entry.trim())
      .filter(Boolean);
  }

  function setDictionary(mode: Mode, raw: string) {
    mode.dictionary = raw
      .split(",")
      .map((entry) => entry.trim())
      .filter(Boolean);
  }

  async function probeFocus() {
    probing = true;
    try {
      focusProbe = await invoke<string>("probe_focused_window");
    } catch (error) {
      focusProbe = `Could not read the focused application (${error})`;
    } finally {
      probing = false;
    }
  }

  function summary(mode: Mode) {
    const bits: string[] = [];
    if (mode.match.app_classes.length) bits.push(mode.match.app_classes.join(", "));
    if (mode.match.title_contains) bits.push(`title ~ "${mode.match.title_contains}"`);
    if (mode.hotkey) bits.push(mode.hotkey);
    return bits.length ? bits.join(" · ") : "No rule — hotkey only";
  }
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Modes</h2>
    <p class="section__lead">
      Per-application overrides. When you start dictating, Oto checks the focused
      application against this list from the top and the first match wins. Anything
      a mode leaves alone comes from your usual settings.
    </p>
  </header>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Identify an application</span>
      <p class="rack__note">
        Focus the application you want a mode for, come back here, and read what Oto sees.
      </p>
    </div>

    <div class="row row--switch" class:row--flush={!focusProbe}>
      <span class="row__copy">
        <strong>Read the focused application</strong>
        <span>Reports the name, bundle id, and title you can match against below.</span>
      </span>
      <button type="button" class="btn" disabled={probing} onclick={() => void probeFocus()}>
        <IconTargetArrow aria-hidden="true" size={14} stroke={1.8} />
        {probing ? "Reading…" : "Read it"}
      </button>
    </div>

    {#if focusProbe}
      <div class="row row--stacked row--flush">
        <span class="row__label">Result</span>
        <pre class="output row__control">{focusProbe}</pre>
      </div>
    {/if}
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">
        Configured
        {#if config.modes.length}
          <button type="button" class="btn btn--small" onclick={addMode}>
            <IconPlus aria-hidden="true" size={13} stroke={1.8} />
            Add a mode
          </button>
        {/if}
      </span>
      {#if config.modes.length > 1}
        <p class="rack__note">Order matters — the first mode that matches is the one that runs.</p>
      {/if}
    </div>

    <div class="row row--stacked row--flush">
      <span class="row__label">
        {#if config.modes.length === 0}
          None yet
        {:else}
          {config.modes.length}
          {config.modes.length === 1 ? "mode" : "modes"}
        {/if}
      </span>
      <div class="row__control">
        {#if config.modes.length === 0}
          <div class="empty">
            <p>No modes yet.</p>
            <p class="empty__detail">
              Worth adding when one application needs different treatment — raw transcripts in a
              terminal, a local model in your password manager, a looser tone in Slack.
            </p>
            <button type="button" class="btn btn--small empty__action" onclick={addMode}>
              <IconPlus aria-hidden="true" size={13} stroke={1.8} />
              Add a mode
            </button>
          </div>
        {:else}
          <div class="items">
            {#each config.modes as mode, index (mode.id)}
              <div class="item mode">
                <div class="mode__head">
                  <input type="checkbox" aria-label="Use {mode.name}" bind:checked={mode.enabled} />
                  <button
                    type="button"
                    class="mode__summary"
                    aria-expanded={expanded === mode.id}
                    onclick={() => (expanded = expanded === mode.id ? null : mode.id)}
                  >
                    <span class="item__title">{mode.name}</span>
                    <span class="mode__rule">{summary(mode)}</span>
                  </button>
                  <div class="mode__controls">
                    <button
                      type="button"
                      class="icon-btn"
                      aria-label="Move {mode.name} earlier"
                      disabled={index === 0}
                      onclick={() => move(index, -1)}
                    >
                      <IconChevronUp aria-hidden="true" size={15} stroke={1.8} />
                    </button>
                    <button
                      type="button"
                      class="icon-btn"
                      aria-label="Move {mode.name} later"
                      disabled={index === config.modes.length - 1}
                      onclick={() => move(index, 1)}
                    >
                      <IconChevronDown aria-hidden="true" size={15} stroke={1.8} />
                    </button>
                    <button
                      type="button"
                      class="icon-btn icon-btn--danger"
                      aria-label="Delete {mode.name}"
                      onclick={() => removeMode(mode.id)}
                    >
                      <IconTrash aria-hidden="true" size={15} stroke={1.8} />
                    </button>
                  </div>
                </div>

                {#if expanded === mode.id}
                  <div class="mode__body">
                    <div class="field-grid field-grid--pair">
                      <label class="field">
                        <span class="plate-micro field__label">Name</span>
                        <input type="text" bind:value={mode.name} />
                      </label>
                      <label class="field">
                        <span class="plate-micro field__label">Its own shortcut</span>
                        <input
                          type="text"
                          class="field-data"
                          placeholder="None"
                          spellcheck="false"
                          autocomplete="off"
                          bind:value={mode.hotkey}
                        />
                      </label>
                    </div>

                    <div class="subrack" role="group" aria-label="Matches">
                      <span class="plate-micro subrack__title">Matches</span>
                      <label class="field">
                        <span class="plate-micro field__label">Applications</span>
                        <input
                          type="text"
                          placeholder="Slack, com.tinyspeck.slackmacgap"
                          spellcheck="false"
                          value={mode.match.app_classes.join(", ")}
                          onchange={(event) => setClasses(mode, event.currentTarget.value)}
                        />
                        <span class="field__hint">
                          Application names or bundle ids, comma separated, matched
                          case-insensitively as a substring. Leave it empty to match any
                          application.
                        </span>
                      </label>
                      <label class="field">
                        <span class="plate-micro field__label">Window title contains</span>
                        <input type="text" placeholder="Anything" bind:value={mode.match.title_contains} />
                      </label>
                      {#if mode.match.app_classes.length === 0 && !mode.match.title_contains.trim()}
                        <p class="field__hint status-warn">
                          With no rule, this mode never matches an application on its own — it runs
                          only from its own shortcut.
                        </p>
                      {/if}
                    </div>

                    <div class="subrack" role="group" aria-label="Overrides">
                      <span class="plate-micro subrack__title">Overrides</span>
                      <div class="field-grid field-grid--pair">
                        <label class="field">
                          <span class="plate-micro field__label">Engine</span>
                          <div class="select-wrap">
                            <select
                              value={mode.stt_backend ?? ""}
                              onchange={(e) =>
                                (mode.stt_backend = (e.currentTarget.value ||
                                  null) as typeof mode.stt_backend)}
                            >
                              <option value="">Same as usual</option>
                              <option value="cloud">Cloud</option>
                              <option value="local_whisper">On this machine</option>
                            </select>
                            <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
                          </div>
                        </label>

                        <label class="field">
                          <span class="plate-micro field__label">Provider</span>
                          <div class="select-wrap">
                            <select
                              value={mode.provider_preset ?? ""}
                              onchange={(e) =>
                                (mode.provider_preset = (e.currentTarget.value ||
                                  null) as typeof mode.provider_preset)}
                            >
                              <option value="">Same as usual</option>
                              <option value="deepgram">Deepgram</option>
                              <option value="open_ai">OpenAI</option>
                              <option value="groq">Groq</option>
                              <option value="open_router">OpenRouter</option>
                              <option value="custom">Custom</option>
                            </select>
                            <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
                          </div>
                        </label>

                        <label class="field">
                          <span class="plate-micro field__label">Speech model</span>
                          <input
                            type="text"
                            class="field-data"
                            placeholder="Same as usual"
                            spellcheck="false"
                            value={mode.stt_model ?? ""}
                            onchange={(e) => (mode.stt_model = e.currentTarget.value || null)}
                          />
                        </label>

                        <label class="field">
                          <span class="plate-micro field__label">Cleanup</span>
                          <div class="select-wrap">
                            <select
                              value={mode.polish_enabled === null ? "" : String(mode.polish_enabled)}
                              onchange={(e) =>
                                (mode.polish_enabled =
                                  e.currentTarget.value === ""
                                    ? null
                                    : e.currentTarget.value === "true")}
                            >
                              <option value="">Same as usual</option>
                              <option value="true">Clean it up</option>
                              <option value="false">Insert the raw transcript</option>
                            </select>
                            <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
                          </div>
                        </label>

                        <label class="field">
                          <span class="plate-micro field__label">Style</span>
                          <div class="select-wrap">
                            <select
                              value={mode.active_style_id ?? ""}
                              onchange={(e) => (mode.active_style_id = e.currentTarget.value || null)}
                            >
                              <option value="">Same as usual</option>
                              {#each config.styles as style (style.id)}
                                <option value={style.id}>{style.name}</option>
                              {/each}
                            </select>
                            <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
                          </div>
                        </label>

                        <label class="field">
                          <span class="plate-micro field__label">Insertion</span>
                          <div class="select-wrap">
                            <select
                              value={mode.injection_mode ?? ""}
                              onchange={(e) =>
                                (mode.injection_mode = (e.currentTarget.value ||
                                  null) as typeof mode.injection_mode)}
                            >
                              <option value="">Same as usual</option>
                              <option value="auto">Auto</option>
                              <option value="direct_type">Typed</option>
                              <option value="clipboard_paste">Clipboard + paste</option>
                              <option value="clipboard_only">Clipboard only</option>
                            </select>
                            <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
                          </div>
                        </label>

                        <label class="field field--span">
                          <span class="plate-micro field__label">What the model is told</span>
                          <div class="select-wrap">
                            <select
                              value={mode.context_level ?? ""}
                              onchange={(e) =>
                                (mode.context_level = (e.currentTarget.value ||
                                  null) as typeof mode.context_level)}
                            >
                              <option value="">Same as usual</option>
                              <option value="none">Nothing</option>
                              <option value="app">The application name</option>
                              <option value="window">Application and window title</option>
                              <option value="selection">Application, title, and nearby text</option>
                            </select>
                            <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
                          </div>
                        </label>

                        <label class="field field--span">
                          <span class="plate-micro field__label">Tone</span>
                          <input
                            type="text"
                            placeholder="Same as usual"
                            value={mode.tone_hint ?? ""}
                            onchange={(e) => (mode.tone_hint = e.currentTarget.value || null)}
                          />
                        </label>

                        <label class="field field--span">
                          <span class="plate-micro field__label">Extra vocabulary</span>
                          <input
                            type="text"
                            placeholder="kubectl, systemd, journalctl"
                            spellcheck="false"
                            value={mode.dictionary.join(", ")}
                            onchange={(e) => setDictionary(mode, e.currentTarget.value)}
                          />
                          <span class="field__hint">
                            Added to your dictionary while this mode is active, not instead of it.
                          </span>
                        </label>
                      </div>
                    </div>
                  </div>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</section>

<style>
  .empty__detail {
    max-width: 44ch;
    margin: 0.375rem auto 0;
    color: var(--faint);
    font-size: var(--text-xs);
    line-height: 1.55;
  }

  .empty__action {
    margin-block-start: var(--space-md);
  }

  .mode {
    gap: 0;
    padding: 0;
  }

  .mode__head {
    display: flex;
    align-items: center;
    gap: 0.625rem;
    padding: 0.625rem 0.75rem;
  }

  .mode__summary {
    display: grid;
    gap: 0.125rem;
    min-width: 0;
    flex: 1;
    border: 0;
    padding: 0;
    background: transparent;
    text-align: start;
  }

  .mode__rule {
    overflow: hidden;
    color: var(--muted);
    font-size: var(--text-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mode__controls {
    display: flex;
    align-items: center;
    gap: 1px;
    flex: 0 0 auto;
  }

  .mode__body {
    display: grid;
    gap: 0.6875rem;
    padding: 0.75rem;
    border-block-start: var(--rule) solid var(--etch);
    background: var(--chassis);
  }
</style>
