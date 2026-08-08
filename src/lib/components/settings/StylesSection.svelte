<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { IconChevronDown } from "@tabler/icons-svelte";
  import type { AppConfig, StylePreset } from "$lib/types";

  let { config = $bindable() }: { config: AppConfig } = $props();
  let commandBusy = $state(false);
  let commandStatus = $state<string | null>(null);

  function patchStyle(id: string, patch: Partial<StylePreset>) {
    config.styles = config.styles.map((style) => style.id === id ? { ...style, ...patch } : style);
  }

  function addStyle() {
    const id = globalThis.crypto?.randomUUID?.() ?? `style-${Date.now()}`;
    config.styles = [...config.styles, { id, name: "Custom style", prompt: "" }];
    config.active_style_id = id;
  }

  async function startCommandMode() {
    commandBusy = true;
    commandStatus = "Refocus the app and keep text selected — capture starts in 2 seconds.";
    try {
      await invoke("set_config", { cfg: config });
      await invoke("start_command_mode", { focusDelayMs: 2000 });
      commandStatus =
        "Listening. Speak the edit instruction, then release your dictation hotkey (or tray → Stop Listening).";
    } catch (error) {
      commandStatus = `Command Mode failed: ${String(error)}`;
    } finally {
      commandBusy = false;
    }
  }
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Styles &amp; commands</h2>
    <p class="section__lead">
      Standing instructions for how your dictation should read, and a way to rewrite
      text you have already selected by saying what you want changed.
    </p>
  </header>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Styles</span>
      <p class="rack__note">
        The active style is added to every cleanup pass. Only one applies at a time.
      </p>
    </div>

    <label class="row">
      <span class="row__label">Active style</span>
      <span class="row__control select-wrap">
        <select
          value={config.active_style_id ?? ""}
          onchange={(event) => (config.active_style_id = event.currentTarget.value || null)}
        >
          <option value="">None</option>
          {#each config.styles as style (style.id)}
            <option value={style.id}>{style.name}</option>
          {/each}
        </select>
        <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
      </span>
    </label>

    <div class="row row--stacked row--flush">
      <span class="row__label">Saved styles</span>
      <div class="row__control">
        {#if config.styles.length === 0}
          <p class="empty">No styles yet.</p>
        {:else}
          <div class="items">
            {#each config.styles as style (style.id)}
              <article class="item">
                <div class="style__head">
                  <input
                    aria-label="Style name"
                    value={style.name}
                    oninput={(event) => patchStyle(style.id, { name: event.currentTarget.value })}
                  />
                  <button
                    type="button"
                    class="btn-link btn-link--danger"
                    onclick={() => {
                      config.styles = config.styles.filter((item) => item.id !== style.id);
                      if (config.active_style_id === style.id) config.active_style_id = null;
                    }}
                  >
                    Remove
                  </button>
                </div>
                <textarea
                  aria-label="How this style should read"
                  rows="2"
                  placeholder="Professional, clear, and concise."
                  value={style.prompt}
                  oninput={(event) => patchStyle(style.id, { prompt: event.currentTarget.value })}
                ></textarea>
              </article>
            {/each}
          </div>
        {/if}
        <button type="button" class="btn btn--small style__add" onclick={addStyle}>Add a style</button>
      </div>
    </div>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Command mode</span>
      <p class="rack__note">
        Select text in any application, start command mode, then say what you want — “make this
        shorter”, “translate this to Spanish”. The rewrite runs through your cleanup model and
        replaces the selection.
      </p>
    </div>

    <div class="row row--switch row--flush">
      <span class="row__copy">
        <strong>Rewrite the current selection</strong>
        <span>You get two seconds to refocus the other window before capture starts.</span>
      </span>
      <button type="button" class="btn" disabled={commandBusy} onclick={startCommandMode}>
        {commandBusy ? "Preparing…" : "Start"}
      </button>
    </div>

    {#if commandStatus}
      <p aria-live="polite" class="note note--warn command-status">{commandStatus}</p>
    {/if}
  </div>
</section>

<style>
  .items {
    margin: 0;
  }

  .style__head {
    display: flex;
    align-items: center;
    gap: var(--space-sm);
    min-width: 0;
  }

  .style__head input {
    min-width: 0;
    flex: 1;
    font-weight: 560;
  }

  .style__add {
    justify-self: start;
    margin-block-start: var(--space-xs);
  }

  .command-status {
    margin-block-start: var(--space-sm);
  }
</style>
