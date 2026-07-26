<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type {
    AppConfig,
    HistoryEntry,
    ReplacementSuggestion,
  } from "$lib/types";

  let entries = $state<HistoryEntry[]>([]);
  let historyEnabled = $state(true);
  let status = $state<string | null>(null);
  let busyId = $state<string | null>(null);
  let clearing = $state(false);
  /** In-app confirm — native window.confirm is unreliable in Tauri WKWebView. */
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
        raw: entry.final_text,
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
  });
</script>

<section class="space-y-6">
  <header class="history-header">
    <div>
      <h2 class="text-xl font-semibold tracking-tight">History</h2>
      <p class="mt-1 text-sm text-slate-400">A local scratchpad of recent dictations and command edits.</p>
    </div>
    {#if entries.length && !confirmClear}
      <button type="button" class="history-clear-btn" onclick={requestClear} disabled={clearing}>
        Clear history
      </button>
    {/if}
  </header>

  {#if confirmClear}
    <div
      class="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-rose-400/25 bg-rose-400/5 px-4 py-3"
      role="alertdialog"
      aria-labelledby="clear-history-title"
      aria-describedby="clear-history-desc"
    >
      <div class="min-w-0">
        <p id="clear-history-title" class="text-sm font-medium text-rose-100">Clear all saved dictations?</p>
        <p id="clear-history-desc" class="mt-0.5 text-xs text-slate-400">This cannot be undone.</p>
      </div>
      <div class="flex shrink-0 gap-2">
        <button type="button" class="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/15" onclick={cancelClear} disabled={clearing}>
          Cancel
        </button>
        <button
          type="button"
          class="rounded-lg bg-rose-500/90 px-3 py-1.5 text-xs font-medium text-white hover:bg-rose-400 disabled:opacity-50"
          onclick={clearAll}
          disabled={clearing}
        >
          {clearing ? "Clearing…" : "Clear history"}
        </button>
      </div>
    </div>
  {/if}

  <div class="space-y-4 rounded-2xl border border-white/10 bg-white/[0.04] p-5">
    <div>
      <h3 class="text-sm font-semibold tracking-tight text-slate-200">Transcribe an audio file</h3>
      <p class="mt-1 text-xs text-slate-500">
        Runs a recording through the same pipeline as dictation and saves the result here. Accepts
        wav, mp3, m4a, ogg, flac, and webm up to 25 MB.
      </p>
    </div>
    <div class="flex gap-2">
      <input
        type="text"
        spellcheck="false"
        placeholder="/Users/you/recording.m4a"
        class="min-w-0 flex-1 rounded-xl border border-white/10 bg-slate-900 px-3 py-2.5 font-mono text-sm"
        bind:value={importPath}
      />
      <button
        type="button"
        class="shrink-0 rounded-xl bg-white/10 px-4 py-2.5 text-sm hover:bg-white/15 disabled:opacity-50"
        disabled={!importPath.trim() || busyId === "import"}
        onclick={() => void importFile()}
      >
        {busyId === "import" ? "Transcribing…" : "Transcribe"}
      </button>
    </div>
  </div>

  {#if entries.length > 1}
    <input
      type="search"
      placeholder="Search transcripts"
      aria-label="Search history"
      class="w-full rounded-xl border border-white/10 bg-slate-900 px-3 py-2.5 text-sm"
      bind:value={query}
    />
  {/if}

  {#if !historyEnabled}
    <p class="rounded-xl border border-amber-400/20 bg-amber-400/5 px-4 py-3 text-sm text-amber-100/90">
      Saving new history is off. Existing entries below remain until deleted. Re-enable under
      <strong>Privacy &amp; sync</strong>.
    </p>
  {/if}

  {#if status}
    <p
      aria-live="polite"
      class="text-sm {status.includes('failed') || status.includes('Failed') || status.includes('still has') ? 'text-rose-300' : 'text-slate-400'}"
    >
      {status}
    </p>
  {/if}

  {#if entries.length === 0}
    <div class="rounded-2xl border border-dashed border-white/15 px-6 py-14 text-center text-sm text-slate-500">
      No saved dictations yet.
    </div>
  {:else if visible.length === 0}
    <div class="rounded-2xl border border-dashed border-white/15 px-6 py-14 text-center text-sm text-slate-500">
      Nothing matches “{query}”.
    </div>
  {:else}
    <div class="space-y-3">
      {#each visible as entry (entry.id)}
        <article class="rounded-2xl border border-white/10 bg-white/[0.04] p-5">
          <div class="mb-3 flex items-center justify-between gap-3 text-xs text-slate-500">
            <span class="rounded-full bg-white/5 px-2 py-1 capitalize">{entry.mode}</span>
            <span class="flex items-center gap-2">
              {#if formatDuration(entry.duration_ms)}
                <span class="tabular-nums">{formatDuration(entry.duration_ms)}</span>
              {/if}
              <time datetime={new Date(entry.created_at_ms).toISOString()}>
                {new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(entry.created_at_ms)}
              </time>
            </span>
          </div>
          <p class="whitespace-pre-wrap text-sm leading-relaxed text-slate-200">{entry.final_text}</p>
          {#if entry.raw_text !== entry.final_text}
            <details class="mt-3 text-xs text-slate-500">
              <summary class="cursor-pointer">Raw transcript</summary>
              <p class="mt-2 whitespace-pre-wrap">{entry.raw_text}</p>
            </details>
          {/if}
          {#if audioUrls[entry.id]}
            <!-- svelte-ignore a11y_media_has_caption -->
            <audio class="mt-3 w-full" controls src={audioUrls[entry.id]}></audio>
          {/if}
          <div class="mt-4 flex flex-wrap gap-2">
            <button type="button" class="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/15" onclick={() => copy(entry.final_text)}>
              Copy
            </button>
            <button
              type="button"
              class="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/15 disabled:opacity-50"
              disabled={busyId === entry.id}
              onclick={() => void reinsert(entry)}
            >
              Insert
            </button>
            {#if entry.has_audio}
              {#if !audioUrls[entry.id]}
                <button
                  type="button"
                  class="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/15 disabled:opacity-50"
                  disabled={busyId === entry.id}
                  onclick={() => void playAudio(entry.id)}
                >
                  Play
                </button>
              {/if}
              <button
                type="button"
                class="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/15 disabled:opacity-50"
                disabled={busyId === entry.id}
                onclick={() => void retranscribe(entry.id)}
              >
                {busyId === entry.id ? "Working…" : "Re-transcribe"}
              </button>
            {/if}
            <button
              type="button"
              class="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/15"
              onclick={() => startCorrection(entry)}
            >
              Teach a correction
            </button>
            <button
              type="button"
              class="rounded-lg px-3 py-1.5 text-xs text-rose-300 hover:bg-white/10 disabled:opacity-50"
              disabled={busyId === entry.id}
              onclick={() => remove(entry.id)}
            >
              Delete
            </button>
          </div>

          {#if correcting === entry.id}
            <div class="mt-4 space-y-3 rounded-xl border border-white/10 bg-slate-950/40 p-4">
              <p class="text-xs leading-relaxed text-slate-400">
                Fix the words Oto got wrong. Consistent single-word corrections can become
                permanent replacement rules — rephrasing and edits are ignored.
              </p>
              <textarea
                rows="4"
                class="w-full rounded-lg border border-white/10 bg-slate-900 px-3 py-2 text-sm text-slate-100"
                bind:value={correction}
              ></textarea>

              {#if suggestions.length}
                <ul class="space-y-1.5">
                  {#each suggestions as suggestion (suggestion.from)}
                    <li class="flex items-center gap-2 text-sm">
                      <input
                        type="checkbox"
                        class="h-4 w-4 shrink-0 rounded border-white/20 bg-slate-900 text-sky-500"
                        checked={accepted[suggestion.from]}
                        onchange={(event) =>
                          (accepted = { ...accepted, [suggestion.from]: event.currentTarget.checked })}
                      />
                      <code class="rounded bg-white/5 px-1.5 py-0.5 text-xs text-rose-200">{suggestion.from}</code>
                      <span aria-hidden="true" class="text-slate-600">→</span>
                      <code class="rounded bg-white/5 px-1.5 py-0.5 text-xs text-emerald-200">{suggestion.to}</code>
                    </li>
                  {/each}
                </ul>
              {/if}

              <div class="flex flex-wrap gap-2">
                {#if suggestions.length}
                  <button
                    type="button"
                    class="rounded-lg bg-sky-500 px-3 py-1.5 text-xs font-medium text-white hover:bg-sky-400 disabled:opacity-50"
                    disabled={busyId === entry.id || !Object.values(accepted).some(Boolean)}
                    onclick={() => void saveSuggestions()}
                  >
                    Save as rules
                  </button>
                {:else}
                  <button
                    type="button"
                    class="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/15 disabled:opacity-50"
                    disabled={busyId === entry.id || correction === entry.final_text}
                    onclick={() => void findSuggestions(entry)}
                  >
                    {busyId === entry.id ? "Comparing…" : "Find corrections"}
                  </button>
                {/if}
                <button
                  type="button"
                  class="rounded-lg px-3 py-1.5 text-xs text-slate-400 hover:bg-white/10"
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
</section>

