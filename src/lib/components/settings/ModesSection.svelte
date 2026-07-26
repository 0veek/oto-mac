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
      // crypto.randomUUID is available in every WebKitGTK build Tauri 2 ships.
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
      focusProbe = `Could not read the focused window (${error})`;
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

<section class="space-y-6">
  <header>
    <h2 class="text-xl font-semibold tracking-tight">Modes</h2>
    <p class="mt-1 text-sm text-slate-400">
      Per-application overrides. When you start dictating, Oto matches the focused window against
      this list top to bottom and the first match wins; anything a mode does not set is inherited
      from your global settings.
    </p>
  </header>

  <div class="space-y-4 rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl">
    <div class="flex items-start justify-between gap-4">
      <div>
        <h3 class="text-sm font-semibold tracking-tight text-slate-200">Identify an application</h3>
        <p class="mt-1 text-xs text-slate-500">
          Focus the application you want a mode for, then come back and press this to see the name,
          bundle id, and title Oto sees.
        </p>
      </div>
      <button
        type="button"
        class="flex shrink-0 items-center gap-1.5 rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-medium text-slate-200 transition hover:bg-white/10 disabled:opacity-50"
        disabled={probing}
        onclick={() => void probeFocus()}
      >
        <IconTargetArrow aria-hidden="true" size={14} stroke={1.8} />
        {probing ? "Reading…" : "Read focused application"}
      </button>
    </div>
    {#if focusProbe}
      <pre
        class="overflow-x-auto rounded-xl border border-white/10 bg-slate-950/70 px-4 py-3 font-mono text-xs text-slate-300">{focusProbe}</pre>
    {/if}
  </div>

  {#if config.modes.length === 0}
    <div
      class="rounded-2xl border border-dashed border-white/15 bg-white/[0.02] px-6 py-10 text-center"
    >
      <p class="text-sm text-slate-300">No modes yet.</p>
      <p class="mx-auto mt-1 max-w-md text-xs text-slate-500">
        A mode is useful when one application needs different treatment — raw transcripts in a
        terminal, a local model in your password manager, a chattier tone in Slack.
      </p>
      <button
        type="button"
        class="mt-4 inline-flex items-center gap-1.5 rounded-lg border border-sky-400/30 bg-sky-400/10 px-3 py-1.5 text-xs font-medium text-sky-100 transition hover:bg-sky-400/20"
        onclick={addMode}
      >
        <IconPlus aria-hidden="true" size={14} stroke={1.8} />
        Add a mode
      </button>
    </div>
  {:else}
    <div class="space-y-3">
      {#each config.modes as mode, index (mode.id)}
        <div class="overflow-hidden rounded-2xl border border-white/10 bg-white/[0.04] shadow-xl backdrop-blur-xl">
          <div class="flex items-center gap-3 px-5 py-4">
            <input
              type="checkbox"
              aria-label="Enable {mode.name}"
              class="h-4 w-4 shrink-0 rounded border-white/20 bg-slate-900 text-sky-500 focus:ring-sky-400/30"
              bind:checked={mode.enabled}
            />
            <button
              type="button"
              class="min-w-0 flex-1 text-left"
              onclick={() => (expanded = expanded === mode.id ? null : mode.id)}
            >
              <span class="block truncate text-sm font-medium text-slate-100">{mode.name}</span>
              <span class="block truncate text-xs text-slate-500">{summary(mode)}</span>
            </button>
            <div class="flex shrink-0 items-center gap-1">
              <button
                type="button"
                aria-label="Move up"
                class="rounded-md p-1.5 text-slate-400 transition hover:bg-white/10 hover:text-slate-200 disabled:opacity-30"
                disabled={index === 0}
                onclick={() => move(index, -1)}
              >
                <IconChevronUp aria-hidden="true" size={16} stroke={1.8} />
              </button>
              <button
                type="button"
                aria-label="Move down"
                class="rounded-md p-1.5 text-slate-400 transition hover:bg-white/10 hover:text-slate-200 disabled:opacity-30"
                disabled={index === config.modes.length - 1}
                onclick={() => move(index, 1)}
              >
                <IconChevronDown aria-hidden="true" size={16} stroke={1.8} />
              </button>
              <button
                type="button"
                aria-label="Delete {mode.name}"
                class="rounded-md p-1.5 text-slate-400 transition hover:bg-rose-500/15 hover:text-rose-300"
                onclick={() => removeMode(mode.id)}
              >
                <IconTrash aria-hidden="true" size={16} stroke={1.8} />
              </button>
            </div>
          </div>

          {#if expanded === mode.id}
            <div class="space-y-5 border-t border-white/10 bg-slate-950/30 px-5 py-5">
              <div class="grid gap-4 sm:grid-cols-2">
                <label class="block space-y-1.5">
                  <span class="text-xs font-medium text-slate-400">Name</span>
                  <input
                    type="text"
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 text-sm text-white outline-none focus:border-sky-400/50 focus:ring-2 focus:ring-sky-400/20"
                    bind:value={mode.name}
                  />
                </label>
                <label class="block space-y-1.5">
                  <span class="text-xs font-medium text-slate-400">Dedicated hotkey</span>
                  <input
                    type="text"
                    placeholder="(none)"
                    spellcheck="false"
                    autocomplete="off"
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 font-mono text-sm text-white outline-none focus:border-sky-400/50 focus:ring-2 focus:ring-sky-400/20"
                    bind:value={mode.hotkey}
                  />
                </label>
              </div>

              <fieldset class="space-y-3 rounded-xl border border-white/10 bg-slate-900/40 p-4">
                <legend class="px-1 text-xs font-semibold text-slate-300">Matches</legend>
                <label class="block space-y-1.5">
                  <span class="text-xs font-medium text-slate-400">Applications</span>
                  <input
                    type="text"
                    placeholder="Slack, com.tinyspeck.slackmacgap"
                    spellcheck="false"
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 text-sm text-white outline-none focus:border-sky-400/50 focus:ring-2 focus:ring-sky-400/20"
                    value={mode.match.app_classes.join(", ")}
                    onchange={(event) => setClasses(mode, event.currentTarget.value)}
                  />
                  <span class="block text-xs text-slate-500">
                    Application names or bundle ids, comma separated, matched case-insensitively
                    as a substring. Leave empty to match any application.
                  </span>
                </label>
                <label class="block space-y-1.5">
                  <span class="text-xs font-medium text-slate-400">Window title contains</span>
                  <input
                    type="text"
                    placeholder="(any)"
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 text-sm text-white outline-none focus:border-sky-400/50 focus:ring-2 focus:ring-sky-400/20"
                    bind:value={mode.match.title_contains}
                  />
                </label>
                {#if mode.match.app_classes.length === 0 && !mode.match.title_contains.trim()}
                  <p class="text-xs text-amber-200/80">
                    With no rule this mode never matches a window automatically — it only applies
                    when triggered by its own hotkey.
                  </p>
                {/if}
              </fieldset>

              <fieldset class="grid gap-4 rounded-xl border border-white/10 bg-slate-900/40 p-4 sm:grid-cols-2">
                <legend class="px-1 text-xs font-semibold text-slate-300">Overrides</legend>

                <label class="block space-y-1.5">
                  <span class="text-xs font-medium text-slate-400">Speech-to-text</span>
                  <select
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 text-sm text-white outline-none focus:border-sky-400/50"
                    value={mode.stt_backend ?? ""}
                    onchange={(e) =>
                      (mode.stt_backend = (e.currentTarget.value || null) as typeof mode.stt_backend)}
                  >
                    <option value="">Inherit</option>
                    <option value="cloud">Cloud</option>
                    <option value="local_whisper">Local Whisper</option>
                  </select>
                </label>

                <label class="block space-y-1.5">
                  <span class="text-xs font-medium text-slate-400">Provider</span>
                  <select
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 text-sm text-white outline-none focus:border-sky-400/50"
                    value={mode.provider_preset ?? ""}
                    onchange={(e) =>
                      (mode.provider_preset = (e.currentTarget.value ||
                        null) as typeof mode.provider_preset)}
                  >
                    <option value="">Inherit</option>
                    <option value="deepgram">Deepgram</option>
                    <option value="open_ai">OpenAI</option>
                    <option value="groq">Groq</option>
                    <option value="open_router">OpenRouter</option>
                    <option value="custom">Custom</option>
                  </select>
                </label>

                <label class="block space-y-1.5">
                  <span class="text-xs font-medium text-slate-400">Transcription model</span>
                  <input
                    type="text"
                    placeholder="Inherit"
                    spellcheck="false"
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 font-mono text-sm text-white outline-none focus:border-sky-400/50"
                    value={mode.stt_model ?? ""}
                    onchange={(e) => (mode.stt_model = e.currentTarget.value || null)}
                  />
                </label>

                <label class="block space-y-1.5">
                  <span class="text-xs font-medium text-slate-400">Cleanup</span>
                  <select
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 text-sm text-white outline-none focus:border-sky-400/50"
                    value={mode.polish_enabled === null ? "" : String(mode.polish_enabled)}
                    onchange={(e) =>
                      (mode.polish_enabled =
                        e.currentTarget.value === "" ? null : e.currentTarget.value === "true")}
                  >
                    <option value="">Inherit</option>
                    <option value="true">On</option>
                    <option value="false">Off — insert the raw transcript</option>
                  </select>
                </label>

                <label class="block space-y-1.5">
                  <span class="text-xs font-medium text-slate-400">Style</span>
                  <select
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 text-sm text-white outline-none focus:border-sky-400/50"
                    value={mode.active_style_id ?? ""}
                    onchange={(e) => (mode.active_style_id = e.currentTarget.value || null)}
                  >
                    <option value="">Inherit</option>
                    {#each config.styles as style (style.id)}
                      <option value={style.id}>{style.name}</option>
                    {/each}
                  </select>
                </label>

                <label class="block space-y-1.5">
                  <span class="text-xs font-medium text-slate-400">Insertion</span>
                  <select
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 text-sm text-white outline-none focus:border-sky-400/50"
                    value={mode.injection_mode ?? ""}
                    onchange={(e) =>
                      (mode.injection_mode = (e.currentTarget.value ||
                        null) as typeof mode.injection_mode)}
                  >
                    <option value="">Inherit</option>
                    <option value="auto">Auto</option>
                    <option value="direct_type">Direct type</option>
                    <option value="clipboard_paste">Clipboard + paste</option>
                    <option value="clipboard_only">Clipboard only</option>
                  </select>
                </label>

                <label class="block space-y-1.5">
                  <span class="text-xs font-medium text-slate-400">Context sharing</span>
                  <select
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 text-sm text-white outline-none focus:border-sky-400/50"
                    value={mode.context_level ?? ""}
                    onchange={(e) =>
                      (mode.context_level = (e.currentTarget.value ||
                        null) as typeof mode.context_level)}
                  >
                    <option value="">Inherit</option>
                    <option value="none">Nothing</option>
                    <option value="app">Application name</option>
                    <option value="window">Application and window title</option>
                    <option value="selection">Application, title, and nearby text</option>
                  </select>
                </label>

                <label class="block space-y-1.5 sm:col-span-2">
                  <span class="text-xs font-medium text-slate-400">Tone</span>
                  <input
                    type="text"
                    placeholder="Inherit"
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 text-sm text-white outline-none focus:border-sky-400/50"
                    value={mode.tone_hint ?? ""}
                    onchange={(e) => (mode.tone_hint = e.currentTarget.value || null)}
                  />
                </label>

                <label class="block space-y-1.5 sm:col-span-2">
                  <span class="text-xs font-medium text-slate-400">Extra vocabulary</span>
                  <input
                    type="text"
                    placeholder="kubectl, systemd, journalctl"
                    spellcheck="false"
                    class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2 text-sm text-white outline-none focus:border-sky-400/50"
                    value={mode.dictionary.join(", ")}
                    onchange={(e) => setDictionary(mode, e.currentTarget.value)}
                  />
                  <span class="block text-xs text-slate-500">
                    Added to your global dictionary for this mode, not instead of it.
                  </span>
                </label>
              </fieldset>
            </div>
          {/if}
        </div>
      {/each}
    </div>

    <button
      type="button"
      class="inline-flex items-center gap-1.5 rounded-lg border border-white/10 bg-white/5 px-3 py-1.5 text-xs font-medium text-slate-200 transition hover:bg-white/10"
      onclick={addMode}
    >
      <IconPlus aria-hidden="true" size={14} stroke={1.8} />
      Add a mode
    </button>
  {/if}
</section>
