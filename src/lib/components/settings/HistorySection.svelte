<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { AppConfig, HistoryEntry } from "$lib/types";

  let entries = $state<HistoryEntry[]>([]);
  let historyEnabled = $state(true);
  let status = $state<string | null>(null);
  let busyId = $state<string | null>(null);
  let clearing = $state(false);
  /** In-app confirm — native window.confirm is unreliable in Tauri WKWebView. */
  let confirmClear = $state(false);

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
  {:else}
    <div class="space-y-3">
      {#each entries as entry (entry.id)}
        <article class="rounded-2xl border border-white/10 bg-white/[0.04] p-5">
          <div class="mb-3 flex items-center justify-between gap-3 text-xs text-slate-500">
            <span class="rounded-full bg-white/5 px-2 py-1 capitalize">{entry.mode}</span>
            <time datetime={new Date(entry.created_at_ms).toISOString()}>
              {new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(entry.created_at_ms)}
            </time>
          </div>
          <p class="whitespace-pre-wrap text-sm leading-relaxed text-slate-200">{entry.final_text}</p>
          {#if entry.raw_text !== entry.final_text}
            <details class="mt-3 text-xs text-slate-500">
              <summary class="cursor-pointer">Raw transcript</summary>
              <p class="mt-2 whitespace-pre-wrap">{entry.raw_text}</p>
            </details>
          {/if}
          <div class="mt-4 flex gap-2">
            <button type="button" class="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/15" onclick={() => copy(entry.final_text)}>
              Copy
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
        </article>
      {/each}
    </div>
  {/if}
</section>

