<script lang="ts">
  import type { AppConfig } from "$lib/types";

  let {
    config = $bindable(),
  }: {
    config: AppConfig;
  } = $props();

  let draft = $state("");
  let addError = $state<string | null>(null);

  function addTerm() {
    addError = null;
    const term = draft.trim();
    if (!term) {
      addError = "Enter a word or phrase.";
      return;
    }
    const exists = config.dictionary.some(
      (t) => t.toLowerCase() === term.toLowerCase(),
    );
    if (exists) {
      addError = "Already in the dictionary.";
      return;
    }
    config.dictionary = [...config.dictionary, term];
    draft = "";
  }

  function removeTerm(index: number) {
    config.dictionary = config.dictionary.filter((_, i) => i !== index);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      addTerm();
    }
  }
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Dictionary</h2>
    <p class="section__lead">
      The names, products and jargon Oto keeps getting wrong. Terms nudge the
      recogniser; replacements below are absolute.
    </p>
  </header>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Terms</span>
      <p class="rack__note">
        Passed to the speech engine when “Bias toward your dictionary” is on under Models. How much
        weight an engine gives them varies by provider.
      </p>
    </div>

    <div class="row row--stacked row--flush">
      <span class="row__label">Add a term</span>
      <div class="row__control">
        <div class="btn-row term-add">
          <input
            type="text"
            class="term-add__input"
            placeholder="Kubernetes, Aveek, oklch…"
            spellcheck="false"
            aria-label="New term"
            bind:value={draft}
            onkeydown={onKeydown}
          />
          <button type="button" class="btn" onclick={addTerm}>Add</button>
        </div>
        {#if addError}
          <p class="row__hint status-warn">{addError}</p>
        {/if}

        {#if config.dictionary.length === 0}
          <p class="empty">Nothing here yet. Add a term Oto keeps mishearing.</p>
        {:else}
          <ul class="terms">
            {#each config.dictionary as term, i (term + i)}
              <li class="term">
                <span class="term__text">{term}</span>
                <button
                  type="button"
                  class="btn-link btn-link--danger"
                  aria-label={`Remove ${term}`}
                  onclick={() => removeTerm(i)}
                >
                  Remove
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    </div>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Replacements</span>
      <p class="rack__note">
        Applied to the finished transcript after cleanup, so they always win. Use them for the words
        a model gets wrong every single time.
      </p>
    </div>

    <div class="row row--stacked row--flush">
      <span class="row__label">Rules</span>
      <div class="row__control">
        {#if config.replacements.length === 0}
          <p class="empty">No replacements yet.</p>
        {:else}
          <ul class="items">
            {#each config.replacements as rule (rule.id)}
              <li class="item rule">
                <div class="rule__line">
                  <input
                    type="checkbox"
                    aria-label="Apply this rule"
                    bind:checked={rule.enabled}
                  />
                  <input
                    type="text"
                    placeholder="heard as"
                    spellcheck="false"
                    aria-label="Heard as"
                    bind:value={rule.from}
                  />
                  <span aria-hidden="true" class="rule__arrow">→</span>
                  <input
                    type="text"
                    placeholder="written as"
                    spellcheck="false"
                    aria-label="Written as"
                    bind:value={rule.to}
                  />
                  <button
                    type="button"
                    class="btn-link btn-link--danger"
                    aria-label="Remove this rule"
                    onclick={() =>
                      (config.replacements = config.replacements.filter((r) => r.id !== rule.id))}
                  >
                    Remove
                  </button>
                </div>
                <div class="rule__flags">
                  <label class="rule__flag">
                    <input type="checkbox" bind:checked={rule.whole_word} />
                    Whole words only
                  </label>
                  <label class="rule__flag">
                    <input type="checkbox" bind:checked={rule.case_sensitive} />
                    Match case
                  </label>
                </div>
              </li>
            {/each}
          </ul>
        {/if}

        <button
          type="button"
          class="btn btn--small rule__add"
          onclick={() => {
            config.replacements = [
              ...config.replacements,
              {
                id: crypto.randomUUID(),
                from: "",
                to: "",
                whole_word: true,
                case_sensitive: false,
                enabled: true,
              },
            ];
          }}
        >
          Add a rule
        </button>
      </div>
    </div>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Spoken edits</span>
    </div>

    <label class="row row--switch row--flush">
      <span class="row__copy">
        <strong>Act on corrections you speak</strong>
        <span>
          <span class="readout-tight">scratch that</span> takes back what you just said;
          <span class="readout-tight">new paragraph</span> and
          <span class="readout-tight">new line</span> break the text. Handled before cleanup, so the
          model never sees the words you retracted.
        </span>
      </span>
      <input type="checkbox" bind:checked={config.voice_edits_enabled} />
    </label>
  </div>
</section>

<style>
  .term-add {
    flex-wrap: nowrap;
  }

  .term-add__input {
    min-width: 0;
    flex: 1;
  }

  .terms {
    display: grid;
    gap: 2px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .term {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-sm);
    padding: 0.4375rem 0.625rem;
    border: var(--rule) solid var(--etch);
    border-radius: var(--radius-control);
    background: var(--panel);
  }

  .term__text {
    min-width: 0;
    overflow: hidden;
    color: var(--ink-2);
    font-size: var(--text-sm);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .items {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .rule__line {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    min-width: 0;
  }

  .rule__line input[type="text"] {
    min-width: 0;
    flex: 1;
  }

  .rule__arrow {
    flex: 0 0 auto;
    color: var(--faint);
  }

  .rule__flags {
    display: flex;
    gap: var(--space-md);
    flex-wrap: wrap;
    padding-inline-start: 2.75rem;
  }

  .rule__flag {
    display: flex;
    align-items: center;
    gap: 0.4375rem;
    color: var(--muted);
    font-size: var(--text-xs);
  }

  .rule__add {
    justify-self: start;
    margin-block-start: var(--space-xs);
  }

  @media (max-width: 34rem) {
    .rule__line {
      flex-wrap: wrap;
    }

    .rule__flags {
      padding-inline-start: 0;
    }
  }
</style>
