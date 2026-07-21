<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { AppConfig, InjectionMode } from "$lib/types";

  let {
    config = $bindable(),
  }: {
    config: AppConfig;
  } = $props();

  let testBusy = $state(false);
  let testResult = $state<string | null>(null);
  let testError = $state<string | null>(null);

  type AxStatus = {
    trusted: boolean;
    bundled: boolean;
    displayName: string;
    executablePath: string;
    bundleId: string;
    guidance: string;
  };

  let ax = $state<AxStatus | null>(null);
  let axBusy = $state(false);
  let axError = $state<string | null>(null);

  const MODES: { value: InjectionMode; label: string; hint: string }[] = [
    {
      value: "auto",
      label: "Auto",
      hint: "Accessibility insert, then clipboard + ⌘V, then direct typing, then clipboard only.",
    },
    {
      value: "direct_type",
      label: "Direct type",
      hint: "Type character-by-character with synthetic key events (slower on long text).",
    },
    {
      value: "clipboard_paste",
      label: "Clipboard + paste",
      hint: "Always copy, then simulate ⌘V.",
    },
    {
      value: "clipboard_only",
      label: "Clipboard only",
      hint: "Copy text and prompt you to paste (⌘V).",
    },
  ];

  async function refreshAx() {
    try {
      ax = await invoke<AxStatus>("get_accessibility_status");
      axError = null;
    } catch (e) {
      axError = String(e);
    }
  }

  onMount(() => {
    void refreshAx();
    const id = setInterval(() => {
      void refreshAx();
    }, 2500);
    return () => clearInterval(id);
  });

  async function openAccessibility() {
    axBusy = true;
    axError = null;
    try {
      await invoke("open_accessibility_settings_cmd");
      await invoke("request_accessibility");
      await refreshAx();
    } catch (e) {
      axError = String(e);
    } finally {
      axBusy = false;
    }
  }

  async function revealInFinder() {
    axBusy = true;
    axError = null;
    try {
      await invoke("reveal_app_in_finder");
    } catch (e) {
      axError = String(e);
    } finally {
      axBusy = false;
    }
  }

  async function testInjection() {
    testBusy = true;
    testResult = null;
    testError = null;
    try {
      await invoke("set_config", { cfg: config });
      testResult = await invoke<string>("test_injection");
      await refreshAx();
    } catch (e) {
      testError = String(e);
      await refreshAx();
    } finally {
      testBusy = false;
    }
  }
</script>

<section class="space-y-6">
  <header>
    <h2 class="text-xl font-semibold tracking-tight">Injection</h2>
    <p class="mt-1 text-sm text-slate-400">
      How Oto delivers dictated text into the focused application.
    </p>
  </header>

  <div
    class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl"
  >
    <fieldset class="space-y-3">
      <legend class="text-sm font-medium text-slate-300">Mode</legend>
      {#each MODES as mode (mode.value)}
        <label
          class="flex cursor-pointer items-start gap-3 rounded-xl border border-white/10 bg-slate-900/40 px-4 py-3 transition hover:border-white/20 {config.injection_mode ===
          mode.value
            ? 'ring-1 ring-sky-400/40'
            : ''}"
        >
          <input
            type="radio"
            name="injection_mode"
            class="mt-1 h-4 w-4 border-white/20 bg-slate-900 text-sky-500 focus:ring-sky-400/30"
            value={mode.value}
            checked={config.injection_mode === mode.value}
            onchange={() => {
              config.injection_mode = mode.value;
            }}
          />
          <span>
            <span class="block text-sm font-medium text-slate-200">{mode.label}</span>
            <span class="block text-xs text-slate-500">{mode.hint}</span>
          </span>
        </label>
      {/each}
    </fieldset>

    <div
      class="space-y-3 rounded-xl border px-4 py-3 text-xs leading-relaxed {ax?.trusted
        ? 'border-emerald-400/25 bg-emerald-400/5 text-emerald-100/90'
        : 'border-amber-400/25 bg-amber-400/5 text-amber-100/90'}"
    >
      <div class="flex flex-wrap items-center justify-between gap-2">
        <p class="font-medium {ax?.trusted ? 'text-emerald-200' : 'text-amber-200'}">
          Accessibility: {ax
            ? ax.trusted
              ? "granted"
              : "not granted"
            : "checking…"}
        </p>
        {#if ax}
          <span class="font-mono text-[11px] opacity-80">
            list name: {ax.displayName}{ax.bundled ? "" : " (dev binary)"}
          </span>
        {/if}
      </div>

      {#if ax}
        <p class="whitespace-pre-line">{ax.guidance}</p>
        {#if !ax.trusted}
          <ol class="list-decimal space-y-1.5 pl-4 text-amber-100/85">
            <li>Click <strong>Open Accessibility Settings</strong> below.</li>
            <li>Click the lock and enter your Mac password.</li>
            <li>
              Click <strong>+</strong> (Oto will not always appear automatically).
            </li>
            <li>
              {#if ax.bundled}
                Select <strong>{ax.displayName}.app</strong>
                (or use “Reveal in Finder” and drag it in).
              {:else}
                Press <kbd class="rounded bg-black/30 px-1">⌘⇧G</kbd> in the file picker and
                paste the path below, or use “Reveal in Finder”.
              {/if}
            </li>
            <li>Turn the toggle <strong>ON</strong>, then quit and reopen Oto.</li>
          </ol>
          {#if ax.executablePath}
            <p class="break-all rounded-lg bg-black/20 px-2 py-1.5 font-mono text-[11px] text-amber-50/90">
              {ax.executablePath}
            </p>
          {/if}
        {/if}
      {/if}

      <div class="flex flex-wrap gap-2 pt-1">
        <button
          type="button"
          class="rounded-lg bg-white/10 px-3 py-1.5 text-xs font-medium text-white ring-1 ring-white/15 transition hover:bg-white/15 disabled:opacity-50"
          disabled={axBusy}
          onclick={openAccessibility}
        >
          {axBusy ? "Opening…" : "Open Accessibility Settings"}
        </button>
        <button
          type="button"
          class="rounded-lg bg-white/5 px-3 py-1.5 text-xs font-medium text-white/90 ring-1 ring-white/10 transition hover:bg-white/10 disabled:opacity-50"
          disabled={axBusy}
          onclick={revealInFinder}
        >
          Reveal in Finder
        </button>
        <button
          type="button"
          class="rounded-lg bg-white/5 px-3 py-1.5 text-xs font-medium text-white/90 ring-1 ring-white/10 transition hover:bg-white/10"
          onclick={() => refreshAx()}
        >
          Refresh status
        </button>
      </div>
      {#if axError}
        <p role="alert" class="text-rose-300">{axError}</p>
      {/if}
    </div>

    <div class="space-y-3 border-t border-white/10 pt-4">
      <div>
        <div class="text-sm font-medium text-slate-200">Test insertion</div>
        <p class="mt-0.5 text-xs text-slate-500">
          Click Test, then immediately focus a text field in another app. Oto waits briefly, then
          injects
          <code class="rounded bg-white/5 px-1">Oto injection test</code>
          using the mode above.
        </p>
      </div>
      <button
        type="button"
        class="rounded-xl bg-white/10 px-4 py-2 text-sm font-medium text-white ring-1 ring-white/15 transition hover:bg-white/15 disabled:cursor-not-allowed disabled:opacity-50"
        disabled={testBusy}
        onclick={testInjection}
      >
        {testBusy ? "Testing…" : "Test insertion"}
      </button>
      {#if testResult}
        <p aria-live="polite" class="text-sm text-emerald-400">{testResult}</p>
      {/if}
      {#if testError}
        <p role="alert" class="text-sm text-rose-400">{testError}</p>
      {/if}
    </div>
  </div>
</section>
