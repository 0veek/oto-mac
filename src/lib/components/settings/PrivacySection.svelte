<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { AppConfig } from "$lib/types";

  let { config = $bindable() }: { config: AppConfig } = $props();
  let token = $state("");
  let tokenPresent = $state(false);
  let busy = $state(false);
  let status = $state<string | null>(null);
  $effect(() => { invoke<boolean>("sync_token_present").then((value) => tokenPresent = value).catch(() => tokenPresent = false); });

  async function saveToken() {
    status = null;
    try {
      await invoke("set_sync_token", { token });
      tokenPresent = token.trim().length > 0;
      token = "";
      status = tokenPresent ? "Sync token saved to the keyring." : "Sync token cleared.";
    } catch (error) {
      status = `Failed to save token: ${String(error)}`;
    }
  }
  async function syncNow() {
    busy = true;
    status = null;
    try {
      await invoke("set_config", { cfg: config });
      status = await invoke<string>("sync_now");
    } catch (error) {
      status = `Sync failed: ${String(error)}`;
    } finally {
      busy = false;
    }
  }
</script>

<section class="space-y-6">
  <header><h2 class="text-xl font-semibold tracking-tight">Privacy &amp; sync</h2><p class="mt-1 text-sm text-slate-400">History stays local. Sync is disabled until you configure and explicitly run it.</p></header>
  <div class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6">
    <div>
      <h3 class="text-sm font-semibold tracking-tight text-slate-200">
        What the cleanup model is told
      </h3>
      <p class="mt-1 text-xs text-slate-500">
        Knowing where text is going lets the model format for it — short lines in a chat client,
        prose in an email, no markdown in a terminal. It is also what leaves your machine, so each
        step is opt-in. This applies only when cleanup is enabled.
      </p>
    </div>

    <div class="space-y-2">
      {#each [
        { value: "none", title: "Nothing", detail: "The model is told only what you said." },
        { value: "app", title: "Application name", detail: "For example \"Slack\" or \"Terminal\"." },
        {
          value: "window",
          title: "Application and window title",
          detail: "Titles often contain file paths, channel names, and subject lines.",
        },
        {
          value: "selection",
          title: "Application, title, and nearby text",
          detail: "Reads the selection through the macOS Accessibility API. Best results, most disclosure.",
        },
      ] as const as option (option.value)}
        <label
          class="flex cursor-pointer items-start gap-3 rounded-xl border border-white/10 bg-slate-900/40 px-4 py-3 transition hover:border-white/20"
        >
          <input
            type="radio"
            name="context-level"
            value={option.value}
            class="mt-0.5 h-4 w-4 shrink-0 border-white/20 bg-slate-900 text-sky-500 focus:ring-sky-400/30"
            bind:group={config.context_level}
          />
          <span class="min-w-0">
            <span class="block text-sm font-medium text-slate-200">{option.title}</span>
            <span class="block text-xs text-slate-500">{option.detail}</span>
          </span>
        </label>
      {/each}
    </div>

    <label class="block space-y-1.5">
      <span class="text-sm text-slate-300">Never describe these applications</span>
      <input
        type="text"
        spellcheck="false"
        placeholder="my-journal, banking-app"
        class="w-full rounded-xl border border-white/10 bg-slate-900 px-3 py-2.5 text-sm"
        value={config.context_blocklist.join(", ")}
        onchange={(event) => {
          config.context_blocklist = event.currentTarget.value
            .split(",")
            .map((entry) => entry.trim())
            .filter(Boolean);
        }}
      />
      <span class="block text-xs text-slate-500">
        Comma separated, matched as a substring of the application name or bundle id. Password
        managers, the keychain, and authenticators are always blocked and cannot be removed from
        this list — a blocked application discloses nothing at all, not even its name. Use
        <strong class="text-slate-400">Modes → Read focused application</strong> to see exactly what
        would be sent.
      </span>
    </label>
  </div>

  <div class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6">
    <label class="flex items-center justify-between gap-4"><span><span class="block text-sm font-medium">Save local history</span><span class="block text-xs text-slate-500">Stored under Oto’s local application data directory.</span></span><input type="checkbox" bind:checked={config.history_enabled} /></label>
    <label class="flex cursor-pointer items-center justify-between gap-4" class:opacity-50={!config.history_enabled}>
      <span>
        <span class="block text-sm font-medium">Keep dictation audio</span>
        <span class="block text-xs text-slate-500">
          Lets history replay a recording and re-transcribe it with different settings. Audio for
          entries you delete — or that fall past the limit below — is removed with them.
        </span>
      </span>
      <input type="checkbox" disabled={!config.history_enabled} bind:checked={config.keep_history_audio} />
    </label>
    <label class="block space-y-1.5" class:opacity-50={!config.history_enabled}>
      <span class="text-sm text-slate-300">Maximum entries</span>
      <input
        type="number"
        min="1"
        max="1000"
        disabled={!config.history_enabled}
        class="w-full rounded-xl border border-white/10 bg-slate-900 px-3 py-2.5 text-sm"
        value={config.history_limit}
        oninput={(event) => {
          const next = Number(event.currentTarget.value);
          config.history_limit = Number.isFinite(next)
            ? Math.min(1000, Math.max(1, Math.round(next)))
            : 100;
        }}
      />
    </label>
  </div>
  <div class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6">
    <label class="flex items-center justify-between gap-4"><span><span class="block text-sm font-medium">Enable user-controlled sync</span><span class="block text-xs text-slate-500">Merges dictionary, snippets, and styles through a JSON GET/PUT endpoint.</span></span><input type="checkbox" bind:checked={config.sync.enabled} /></label>
    <label class="block space-y-1.5" class:opacity-50={!config.sync.enabled}><span class="text-sm text-slate-300">HTTPS document endpoint</span><input type="url" disabled={!config.sync.enabled} placeholder="https://example.com/private/oto.json" class="w-full rounded-xl border border-white/10 bg-slate-900 px-3 py-2.5 text-sm" bind:value={config.sync.endpoint} /></label>
    <div class="flex gap-2" class:opacity-50={!config.sync.enabled}><input type="password" disabled={!config.sync.enabled} class="min-w-0 flex-1 rounded-xl border border-white/10 bg-slate-900 px-3 py-2.5 text-sm" placeholder={tokenPresent ? "Replace saved bearer token…" : "Optional bearer token"} bind:value={token} /><button type="button" disabled={!config.sync.enabled} class="rounded-xl bg-white/10 px-4 py-2.5 text-sm hover:bg-white/15" onclick={saveToken}>Save token</button></div>
    <button type="button" disabled={!config.sync.enabled || busy} class="rounded-xl bg-sky-500 px-4 py-2.5 text-sm font-medium text-white hover:bg-sky-400 disabled:opacity-50" onclick={syncNow}>{busy ? "Syncing…" : "Sync now"}</button>
    {#if status}<p aria-live="polite" class="text-sm {status.startsWith('Sync failed') ? 'text-rose-300' : 'text-slate-300'}">{status}</p>{/if}
    <p class="text-xs leading-relaxed text-slate-500">Local values win when an item has the same ID; remote-only items are added. Oto never syncs API keys, history, audio, or provider credentials.</p>
  </div>
</section>
