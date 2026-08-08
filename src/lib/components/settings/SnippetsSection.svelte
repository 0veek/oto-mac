<script lang="ts">
  import type { AppConfig, Snippet } from "$lib/types";

  let { config = $bindable() }: { config: AppConfig } = $props();
  let trigger = $state("");
  let expansion = $state("");
  let error = $state<string | null>(null);

  function id() {
    return globalThis.crypto?.randomUUID?.() ?? `snippet-${Date.now()}`;
  }

  /**
   * Mirror the backend matcher (features/snippets.rs): triggers are compared
   * case-insensitively with edge punctuation stripped, an optional spoken
   * "snippet" prefix removed, and whitespace collapsed. Comparing raw strings
   * let two triggers that match the same utterance coexist, so only the first
   * could ever fire.
   */
  function normalizeTrigger(value: string) {
    return value
      .trim()
      .replace(/^[.,!?:;"']+|[.,!?:;"']+$/g, "")
      .toLowerCase()
      .replace(/^snippet\s+/, "")
      .split(/\s+/)
      .filter(Boolean)
      .join(" ");
  }

  function addSnippet() {
    error = null;
    if (!trigger.trim() || !expansion.trim()) {
      error = "Add both a spoken trigger and its expansion.";
      return;
    }
    const normalized = normalizeTrigger(trigger);
    if (!normalized) {
      error = "That trigger has no words Oto can match.";
      return;
    }
    if (config.snippets.some((item) => normalizeTrigger(item.trigger) === normalized)) {
      error = "A snippet with an equivalent trigger already exists.";
      return;
    }
    config.snippets = [
      ...config.snippets,
      { id: id(), trigger: trigger.trim(), expansion: expansion.trim(), enabled: true },
    ];
    trigger = "";
    expansion = "";
  }

  function patchSnippet(id: string, patch: Partial<Snippet>) {
    config.snippets = config.snippets.map((item) => item.id === id ? { ...item, ...patch } : item);
  }
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Snippets</h2>
    <p class="section__lead">
      Say a trigger on its own and Oto inserts the block behind it, word for word.
      Saying “snippet” first is optional.
    </p>
  </header>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">New snippet</span>
      <p class="rack__note">
        A trigger only fires when it is the whole utterance, so the same phrase inside ordinary
        dictation stays as it is.
      </p>
    </div>

    <div class="row row--stacked row--flush">
      <span class="row__label">Trigger and text</span>
      <div class="row__control">
        <div class="compose">
          <input
            aria-label="Spoken trigger"
            placeholder="my signature"
            bind:value={trigger}
            onkeydown={(event) => {
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                addSnippet();
              }
            }}
          />
          <textarea
            aria-label="Text to insert"
            rows="1"
            placeholder="Best,&#10;Your name"
            bind:value={expansion}
          ></textarea>
          <button type="button" class="btn" onclick={addSnippet}>Add</button>
        </div>
        {#if error}<p class="row__hint status-warn">{error}</p>{/if}
      </div>
    </div>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Saved</span>
    </div>

    <div class="row row--stacked row--flush">
      <span class="row__label">
        {config.snippets.length}
        {config.snippets.length === 1 ? "snippet" : "snippets"}
      </span>
      <div class="row__control">
        {#if config.snippets.length === 0}
          <p class="empty">No snippets yet.</p>
        {:else}
          <div class="items">
            {#each config.snippets as snippet (snippet.id)}
              <article class="item snippet">
                <input
                  aria-label="Spoken trigger"
                  value={snippet.trigger}
                  oninput={(event) => patchSnippet(snippet.id, { trigger: event.currentTarget.value })}
                />
                <textarea
                  aria-label="Text to insert"
                  rows="2"
                  value={snippet.expansion}
                  oninput={(event) =>
                    patchSnippet(snippet.id, { expansion: event.currentTarget.value })}
                ></textarea>
                <div class="snippet__actions">
                  <label class="snippet__toggle">
                    <input
                      type="checkbox"
                      checked={snippet.enabled}
                      onchange={(event) =>
                        patchSnippet(snippet.id, { enabled: event.currentTarget.checked })}
                    />
                    On
                  </label>
                  <button
                    type="button"
                    class="btn-link btn-link--danger"
                    onclick={() =>
                      (config.snippets = config.snippets.filter((item) => item.id !== snippet.id))}
                  >
                    Remove
                  </button>
                </div>
              </article>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</section>

<style>
  .compose {
    display: grid;
    gap: var(--space-xs);
  }

  .snippet {
    gap: var(--space-xs);
  }

  .snippet__actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-sm);
  }

  .snippet__toggle {
    display: flex;
    align-items: center;
    gap: 0.4375rem;
    color: var(--muted);
    font-size: var(--text-xs);
  }

  @media (min-width: 40rem) {
    .compose {
      grid-template-columns: minmax(0, 0.8fr) minmax(0, 1.4fr) auto;
      align-items: start;
    }

    .snippet {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1.5fr);
      align-items: start;
    }

    .snippet__actions {
      grid-column: 1 / -1;
    }
  }
</style>
