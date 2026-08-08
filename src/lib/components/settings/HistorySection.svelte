<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type {
    AppConfig,
    HistoryEntry,
    PipelineEvent,
    ReplacementSuggestion,
  } from "$lib/types";

  let entries = $state<HistoryEntry[]>([]);
  let historyEnabled = $state(true);
  let status = $state<string | null>(null);
  let busyId = $state<string | null>(null);
  let clearing = $state(false);
  /** In-app confirm — native window.confirm is unreliable in Tauri's WKWebView. */
  let confirmClear = $state(false);
  let query = $state("");
  let audioUrls = $state<Record<string, string>>({});
  let importPath = $state("");

  // Searching a scratchpad is the whole point of keeping one; match both the
  // cleaned text and the raw transcript so a mis-transcribed word still finds it.
  let visible = $derived.by(() => {
    const needle = query.trim().toLocaleLowerCase();
    if (!needle) return entries;
    return entries.filter(
      (entry) =>
        entry.final_text.toLocaleLowerCase().includes(needle) ||
        entry.raw_text.toLocaleLowerCase().includes(needle),
    );
  });

  async function playAudio(id: string) {
    busyId = id;
    status = null;
    try {
      audioUrls = { ...audioUrls, [id]: await invoke<string>("get_history_audio", { id }) };
    } catch (error) {
      status = `Could not load audio: ${String(error)}`;
    } finally {
      busyId = null;
    }
  }

  async function retranscribe(id: string) {
    busyId = id;
    status = null;
    try {
      const text = await invoke<string>("retranscribe_history", { id });
      status = `Re-transcribed: ${text}`;
    } catch (error) {
      status = `Re-transcribe failed: ${String(error)}`;
    } finally {
      busyId = null;
    }
  }

  async function reinsert(entry: HistoryEntry) {
    busyId = entry.id;
    status = "Focus the target window — inserting in about a second…";
    try {
      await invoke("reinsert_history", { text: entry.final_text });
      status = "Inserted.";
    } catch (error) {
      status = `Insert failed: ${String(error)}`;
    } finally {
      busyId = null;
    }
  }

  async function importFile() {
    const path = importPath.trim();
    if (!path) return;
    busyId = "import";
    status = "Transcribing…";
    try {
      const text = await invoke<string>("transcribe_audio_file", { path });
      status = `Transcribed and saved to history: ${text}`;
      importPath = "";
      await refresh();
    } catch (error) {
      status = `Transcription failed: ${String(error)}`;
    } finally {
      busyId = null;
    }
  }

  let correcting = $state<string | null>(null);
  let correction = $state("");
  let suggestions = $state<ReplacementSuggestion[]>([]);
  let accepted = $state<Record<string, boolean>>({});

  function startCorrection(entry: HistoryEntry) {
    correcting = entry.id;
    correction = entry.final_text;
    suggestions = [];
    accepted = {};
    status = null;
  }

  async function findSuggestions(entry: HistoryEntry) {
    busyId = entry.id;
    status = null;
    try {
      suggestions = await invoke<ReplacementSuggestion[]>("suggest_replacements", {
        raw: entry.raw_text,
        corrected: correction,
      });
      accepted = Object.fromEntries(suggestions.map((s) => [s.from, true]));
      if (suggestions.length === 0) {
        status =
          "No repeatable single-word corrections found. Rules are only learned from consistent word-for-word fixes.";
      }
    } catch (error) {
      status = `Could not compare: ${String(error)}`;
    } finally {
      busyId = null;
    }
  }

  async function saveSuggestions() {
    const rules = suggestions
      .filter((s) => accepted[s.from])
      .map((s) => ({
        id: crypto.randomUUID(),
        from: s.from,
        to: s.to,
        whole_word: true,
        case_sensitive: false,
        enabled: true,
      }));
    if (rules.length === 0) return;
    busyId = correcting;
    try {
      const added = await invoke<number>("add_replacement_rules", { rules });
      status =
        added > 0
          ? `Added ${added} replacement ${added === 1 ? "rule" : "rules"}. Future dictations will apply ${added === 1 ? "it" : "them"} automatically.`
          : "Those words already have replacement rules.";
      correcting = null;
      suggestions = [];
    } catch (error) {
      status = `Could not save: ${String(error)}`;
    } finally {
      busyId = null;
    }
  }

  function formatDuration(ms: number) {
    if (!ms) return null;
    const seconds = Math.round(ms / 1000);
    if (seconds < 60) return `${seconds}s`;
    return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
  }

  async function refresh() {
    try {
      const [list, config] = await Promise.all([
        invoke<HistoryEntry[]>("get_history"),
        invoke<AppConfig>("get_config").catch(() => null),
      ]);
      entries = list;
      if (config) historyEnabled = config.history_enabled;
    } catch (error) {
      status = String(error);
    }
  }

  async function remove(id: string) {
    busyId = id;
    status = null;
    try {
      await invoke("delete_history_entry", { id });
      await refresh();
    } catch (error) {
      status = `Delete failed: ${String(error)}`;
    } finally {
      busyId = null;
    }
  }

  function requestClear() {
    status = null;
    confirmClear = true;
  }

  function cancelClear() {
    confirmClear = false;
  }

  async function clearAll() {
    if (clearing) return;
    clearing = true;
    status = null;
    try {
      await invoke("clear_history");
      // Re-read from disk so the UI cannot diverge from storage.
      await refresh();
      if (entries.length === 0) {
        confirmClear = false;
        status = "History cleared.";
      } else {
        status = "Clear finished but history still has entries. Try again.";
      }
    } catch (error) {
      status = `Clear failed: ${String(error)}`;
    } finally {
      clearing = false;
    }
  }

  async function copy(text: string) {
    status = null;
    try {
      await invoke("copy_history_text", { text });
      status = "Copied to clipboard.";
    } catch (error) {
      status = `Copy failed: ${String(error)}`;
    }
  }

  onMount(() => {
    void refresh();
    // A dictation finished while this panel was open — pull the new entry in
    // instead of showing a stale list until the user navigates away and back.
    const unlisten = listen<PipelineEvent>("pipeline://event", ({ payload }) => {
      if (payload.type === "state" && payload.state === "done") {
        void refresh();
      }
    }).catch(() => null);
    return () => {
      void unlisten.then((stop) => stop?.());
    };
  });
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">History</h2>
    <p class="section__lead">
      Everything you have dictated on this machine, kept locally so you can copy it
      again, insert it somewhere else, or teach Oto what it misheard.
    </p>
  </header>

  {#if !historyEnabled}
    <p class="note note--warn">
      New dictations are no longer being saved. What is already here stays until you delete it.
      Turn saving back on under <strong>Privacy</strong>.
    </p>
  {/if}

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">From a file</span>
      <p class="rack__note">
        Runs a recording through the same path as a live dictation and saves the result here. Takes
        wav, mp3, m4a, ogg, flac and webm up to 25 MB.
      </p>
    </div>

    <div class="row row--flush">
      <span class="row__label">Audio file</span>
      <div class="row__control">
        <div class="btn-row import">
          <input
            type="text"
            class="field-data import__input"
            spellcheck="false"
            aria-label="Path to an audio file"
            placeholder="/Users/you/recording.m4a"
            bind:value={importPath}
          />
          <button
            type="button"
            class="btn"
            disabled={!importPath.trim() || busyId === "import"}
            onclick={() => void importFile()}
          >
            {busyId === "import" ? "Transcribing…" : "Transcribe"}
          </button>
        </div>
      </div>
    </div>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">
        Saved
        {#if entries.length && !confirmClear}
          <button
            type="button"
            class="btn-link btn-link--danger"
            disabled={clearing}
            onclick={requestClear}
          >
            Delete all
          </button>
        {/if}
      </span>
    </div>

    {#if confirmClear}
      <!-- Inline rather than window.confirm, which WKWebView will not reliably
           show from a Tauri page. -->
      <div
        class="clear-confirm"
        role="alertdialog"
        aria-labelledby="clear-history-title"
        aria-describedby="clear-history-desc"
      >
        <div class="clear-confirm__copy">
          <strong id="clear-history-title">Delete all saved dictations?</strong>
          <span id="clear-history-desc">This cannot be undone.</span>
        </div>
        <div class="btn-row clear-confirm__actions">
          <button
            type="button"
            class="btn btn--small btn--quiet"
            disabled={clearing}
            onclick={cancelClear}
          >
            Cancel
          </button>
          <button
            type="button"
            class="btn btn--small btn--danger"
            disabled={clearing}
            onclick={() => void clearAll()}
          >
            {clearing ? "Deleting…" : "Delete all"}
          </button>
        </div>
      </div>
    {/if}

    <div class="row row--stacked row--flush">
      <span class="row__label">
        {entries.length}
        {entries.length === 1 ? "dictation" : "dictations"}
      </span>
      <div class="row__control">
        {#if entries.length > 1}
          <input
            type="search"
            placeholder="Search transcripts"
            aria-label="Search history"
            bind:value={query}
          />
        {/if}

        {#if status}
          <p
            aria-live="polite"
            class="row__hint"
            class:status-bad={status.toLowerCase().includes("failed") ||
              status.includes("still has")}
          >
            {status}
          </p>
        {/if}

        {#if entries.length === 0}
          <p class="empty">Nothing saved yet.</p>
        {:else if visible.length === 0}
          <p class="empty">Nothing matches “{query}”.</p>
        {:else}
          <div class="items">
            {#each visible as entry (entry.id)}
              <article class="item entry">
                <div class="item__head">
                  <span class="plate-micro entry__kind">{entry.mode}</span>
                  <span class="item__meta">
                    {#if formatDuration(entry.duration_ms)}
                      {formatDuration(entry.duration_ms)} ·
                    {/if}
                    <time datetime={new Date(entry.created_at_ms).toISOString()}>
                      {new Intl.DateTimeFormat(undefined, {
                        dateStyle: "medium",
                        timeStyle: "short",
                      }).format(entry.created_at_ms)}
                    </time>
                  </span>
                </div>

                <p class="entry__text">{entry.final_text}</p>

                {#if entry.raw_text !== entry.final_text}
                  <details class="disclosure">
                    <summary>What Oto heard first</summary>
                    <div class="disclosure__body">
                      <p class="entry__raw">{entry.raw_text}</p>
                    </div>
                  </details>
                {/if}

                {#if audioUrls[entry.id]}
                  <!-- svelte-ignore a11y_media_has_caption -->
                  <audio class="entry__audio" controls src={audioUrls[entry.id]}></audio>
                {/if}

                <div class="btn-row">
                  <button
                    type="button"
                    class="btn btn--small"
                    onclick={() => copy(entry.final_text)}
                  >
                    Copy
                  </button>
                  <button
                    type="button"
                    class="btn btn--small"
                    disabled={busyId === entry.id}
                    onclick={() => void reinsert(entry)}
                  >
                    Insert
                  </button>
                  {#if entry.has_audio}
                    {#if !audioUrls[entry.id]}
                      <button
                        type="button"
                        class="btn btn--small"
                        disabled={busyId === entry.id}
                        onclick={() => void playAudio(entry.id)}
                      >
                        Play
                      </button>
                    {/if}
                    <button
                      type="button"
                      class="btn btn--small"
                      disabled={busyId === entry.id}
                      onclick={() => void retranscribe(entry.id)}
                    >
                      {busyId === entry.id ? "Working…" : "Transcribe again"}
                    </button>
                  {/if}
                  <button
                    type="button"
                    class="btn btn--small"
                    onclick={() => startCorrection(entry)}
                  >
                    Correct it
                  </button>
                  <button
                    type="button"
                    class="btn btn--small btn--danger"
                    disabled={busyId === entry.id}
                    onclick={() => remove(entry.id)}
                  >
                    Delete
                  </button>
                </div>

                {#if correcting === entry.id}
                  <div class="subrack">
                    <p class="field__hint">
                      Fix the words Oto got wrong. Single words you correct the same way each time
                      can become permanent rules; rewording is ignored.
                    </p>
                    <textarea rows="4" aria-label="Corrected text" bind:value={correction}></textarea>

                    {#if suggestions.length}
                      <ul class="fixes">
                        {#each suggestions as suggestion (suggestion.from)}
                          <li class="fix">
                            <input
                              type="checkbox"
                              aria-label={`Always write ${suggestion.from} as ${suggestion.to}`}
                              checked={accepted[suggestion.from]}
                              onchange={(event) =>
                                (accepted = {
                                  ...accepted,
                                  [suggestion.from]: event.currentTarget.checked,
                                })}
                            />
                            <span class="readout-tight status-bad">{suggestion.from}</span>
                            <span aria-hidden="true" class="fix__arrow">→</span>
                            <span class="readout-tight status-ok">{suggestion.to}</span>
                          </li>
                        {/each}
                      </ul>
                    {/if}

                    <div class="btn-row">
                      {#if suggestions.length}
                        <button
                          type="button"
                          class="btn btn--small btn--primary"
                          disabled={busyId === entry.id ||
                            !Object.values(accepted).some(Boolean)}
                          onclick={() => void saveSuggestions()}
                        >
                          Always write it this way
                        </button>
                      {:else}
                        <button
                          type="button"
                          class="btn btn--small"
                          disabled={busyId === entry.id || correction === entry.final_text}
                          onclick={() => void findSuggestions(entry)}
                        >
                          {busyId === entry.id ? "Comparing…" : "Find what changed"}
                        </button>
                      {/if}
                      <button
                        type="button"
                        class="btn btn--small btn--quiet"
                        onclick={() => {
                          correcting = null;
                          suggestions = [];
                        }}
                      >
                        Cancel
                      </button>
                    </div>
                  </div>
                {/if}
              </article>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>
</section>

<style>
  .import {
    flex-wrap: nowrap;
  }

  .import__input {
    min-width: 0;
    flex: 1;
  }

  /* The confirm is part of the page, so it borrows the fault-note language — a
     clip-red edge over a clip wash — rather than pretending to be a dialog. */
  .clear-confirm {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-md);
    flex-wrap: wrap;
    margin-block-start: 0.875rem;
    padding: 0.625rem 0.75rem;
    border: var(--rule) solid var(--etch);
    border-inline-start: 2px solid var(--signal-clip);
    border-radius: var(--radius-control);
    background: var(--signal-clip-wash);
  }

  .clear-confirm__copy {
    display: grid;
    gap: 0.125rem;
    min-width: 0;
  }

  .clear-confirm__copy strong {
    color: var(--ink-2);
    font-size: var(--text-sm);
    font-weight: 560;
  }

  .clear-confirm__copy span {
    color: var(--muted);
    font-size: var(--text-xs);
    line-height: 1.5;
  }

  .clear-confirm__actions {
    flex-wrap: nowrap;
  }

  .entry__kind {
    color: var(--faint);
  }

  .entry__text {
    color: var(--ink-2);
    font-size: var(--text-sm);
    line-height: 1.6;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .entry__raw {
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .entry__audio {
    width: 100%;
    height: 2rem;
  }

  .fixes {
    display: grid;
    gap: 0.375rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .fix {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: var(--text-sm);
  }

  .fix__arrow {
    color: var(--faint);
  }

  @media (max-width: 30rem) {
    .import {
      flex-wrap: wrap;
    }

    .import__input {
      flex-basis: 100%;
    }
  }
</style>
