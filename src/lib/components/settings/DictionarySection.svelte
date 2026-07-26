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

<section class="space-y-6">
  <header>
    <h2 class="text-xl font-semibold tracking-tight">Dictionary</h2>
    <p class="mt-1 text-sm text-slate-400">
      Names, product terms, and jargon used for both STT vocabulary prompting and polishing.
    </p>
  </header>

  <div
    class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl"
  >
    <div class="flex flex-col gap-2 sm:flex-row">
      <input
        type="text"
        class="min-w-0 flex-1 rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2.5 text-sm text-white outline-none transition placeholder:text-slate-600 focus:border-sky-400/50 focus:ring-2 focus:ring-sky-400/20"
        placeholder="Add a term…"
        spellcheck="false"
        bind:value={draft}
        onkeydown={onKeydown}
      />
      <button
        type="button"
        class="shrink-0 rounded-xl bg-sky-500/90 px-4 py-2.5 text-sm font-medium text-white transition hover:bg-sky-400"
        onclick={addTerm}
      >
        Add
      </button>
    </div>
    {#if addError}
      <p class="text-xs text-amber-300/90">{addError}</p>
    {/if}

    {#if config.dictionary.length === 0}
      <p
        class="rounded-xl border border-dashed border-white/15 bg-slate-900/30 px-4 py-8 text-center text-sm text-slate-500"
      >
        No terms yet. Add names or domain vocabulary you want polish to preserve.
      </p>
    {:else}
      <ul class="space-y-2">
        {#each config.dictionary as term, i (term + i)}
          <li
            class="flex items-center justify-between gap-3 rounded-xl border border-white/10 bg-slate-900/40 px-4 py-2.5"
          >
            <span class="truncate text-sm text-slate-200">{term}</span>
            <button
              type="button"
              class="shrink-0 rounded-lg px-2 py-1 text-xs text-slate-400 transition hover:bg-white/10 hover:text-rose-300"
              onclick={() => removeTerm(i)}
            >
              Remove
            </button>
          </li>
        {/each}
      </ul>
    {/if}

    <p class="text-xs leading-relaxed text-slate-500">
      With vocabulary boosting enabled, terms are passed to the active transcription engine.
      Cloud engines may interpret prompt support differently.
    </p>
  </div>
  <div class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl">
    <div>
      <h3 class="text-sm font-semibold tracking-tight text-slate-200">Replacements</h3>
      <p class="mt-1 text-xs leading-relaxed text-slate-500">
        Applied to the finished transcript, after cleanup, so they are the last word on spelling.
        Use these for the words a model gets wrong every single time; the dictionary above only
        nudges the recogniser, while these always apply.
      </p>
    </div>

    {#if config.replacements.length === 0}
      <p class="rounded-xl border border-dashed border-white/15 bg-slate-900/30 px-4 py-8 text-center text-sm text-slate-500">
        No replacements yet.
      </p>
    {:else}
      <ul class="space-y-2">
        {#each config.replacements as rule (rule.id)}
          <li class="space-y-3 rounded-xl border border-white/10 bg-slate-900/40 px-4 py-3">
            <div class="flex items-center gap-2">
              <input
                type="checkbox"
                aria-label="Enable this replacement"
                class="h-4 w-4 shrink-0 rounded border-white/20 bg-slate-900 text-sky-500"
                bind:checked={rule.enabled}
              />
              <input
                type="text"
                placeholder="heard as"
                spellcheck="false"
                class="min-w-0 flex-1 rounded-lg border border-white/10 bg-slate-900 px-2.5 py-1.5 text-sm"
                bind:value={rule.from}
              />
              <span aria-hidden="true" class="shrink-0 text-slate-600">→</span>
              <input
                type="text"
                placeholder="written as"
                spellcheck="false"
                class="min-w-0 flex-1 rounded-lg border border-white/10 bg-slate-900 px-2.5 py-1.5 text-sm"
                bind:value={rule.to}
              />
              <button
                type="button"
                aria-label="Remove replacement"
                class="shrink-0 rounded-lg px-2 py-1 text-xs text-slate-400 transition hover:bg-white/10 hover:text-rose-300"
                onclick={() => (config.replacements = config.replacements.filter((r) => r.id !== rule.id))}
              >
                Remove
              </button>
            </div>
            <div class="flex gap-4 pl-6 text-xs text-slate-500">
              <label class="flex cursor-pointer items-center gap-1.5">
                <input type="checkbox" class="h-3.5 w-3.5 rounded border-white/20 bg-slate-900 text-sky-500" bind:checked={rule.whole_word} />
                Whole word only
              </label>
              <label class="flex cursor-pointer items-center gap-1.5">
                <input type="checkbox" class="h-3.5 w-3.5 rounded border-white/20 bg-slate-900 text-sky-500" bind:checked={rule.case_sensitive} />
                Match case
              </label>
            </div>
          </li>
        {/each}
      </ul>
    {/if}

    <button
      type="button"
      class="rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-medium text-slate-200 transition hover:bg-white/10"
      onclick={() => {
        config.replacements = [
          ...config.replacements,
          { id: crypto.randomUUID(), from: "", to: "", whole_word: true, case_sensitive: false, enabled: true },
        ];
      }}
    >
      Add replacement
    </button>
  </div>

  <div class="space-y-4 rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl">
    <div>
      <h3 class="text-sm font-semibold tracking-tight text-slate-200">Spoken edits</h3>
      <p class="mt-1 text-xs leading-relaxed text-slate-500">
        Correct yourself out loud and have the correction applied instead of transcribed.
      </p>
    </div>
    <label class="flex cursor-pointer items-center justify-between gap-4 rounded-xl border border-white/10 bg-slate-900/40 px-4 py-3 transition hover:border-white/20">
      <span>
        <span class="block text-sm font-medium text-slate-200">Honour spoken editing commands</span>
        <span class="block text-xs text-slate-500">
          <code class="rounded bg-white/5 px-1">scratch that</code> retracts what you just said;
          <code class="rounded bg-white/5 px-1">new paragraph</code> and
          <code class="rounded bg-white/5 px-1">new line</code> insert breaks. Applied before
          cleanup, so the model never sees the retracted words.
        </span>
      </span>
      <input type="checkbox" class="h-4 w-4 shrink-0 rounded border-white/20 bg-slate-900 text-sky-500" bind:checked={config.voice_edits_enabled} />
    </label>
  </div>
</section>
