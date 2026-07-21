<script lang="ts">
  import type { AppConfig } from "$lib/types";

  let {
    config = $bindable(),
  }: {
    config: AppConfig;
  } = $props();
</script>

<section class="space-y-6">
  <header>
    <h2 class="text-xl font-semibold tracking-tight">Hotkeys</h2>
    <p class="mt-1 text-sm text-slate-400">
      Global push-to-talk shortcut. Hold to dictate, then release to stop and process.
      macOS registers the chord through the system global-shortcut API.
    </p>
  </header>

  <div
    class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl"
  >
    <label class="block space-y-1.5">
      <span class="text-sm font-medium text-slate-300">Dictation hotkey</span>
      <input
        type="text"
        class="w-full rounded-xl border border-white/10 bg-slate-900/80 px-3 py-2.5 font-mono text-sm text-white outline-none transition placeholder:text-slate-600 focus:border-sky-400/50 focus:ring-2 focus:ring-sky-400/20"
        placeholder="Ctrl+Shift+Space"
        spellcheck="false"
        autocomplete="off"
        bind:value={config.hotkey}
      />
      <span class="block text-xs text-slate-500">
        Format: modifiers and a key joined by
        <code class="rounded bg-white/5 px-1">+</code>
        (e.g.
        <code class="rounded bg-white/5 px-1">Ctrl+Shift+Space</code>,
        <code class="rounded bg-white/5 px-1">Cmd+Shift+D</code>).
      </span>
      <span class="block text-xs text-slate-500">
        Supported modifiers: Ctrl, Cmd/Super/Meta, Option/Alt, Shift. Keys: Space, Enter, Tab,
        Escape, a–z.
      </span>
    </label>

    <div class="rounded-xl border border-amber-400/20 bg-amber-400/5 px-4 py-3 text-xs leading-relaxed text-amber-100/90">
      <p class="font-medium text-amber-200">If the overlay does not appear</p>
      <ul class="mt-1.5 list-disc space-y-1 pl-4 text-amber-100/80">
        <li>Click <strong>Save</strong> after changing the hotkey (bottom of Settings).</li>
        <li>
          Grant <strong>Accessibility</strong> (and if prompted, Input Monitoring) to Oto under
          System Settings → Privacy &amp; Security.
        </li>
        <li>
          Avoid reserved combos like
          <code class="rounded bg-white/5 px-1">Cmd+Space</code>
          (Spotlight) and
          <code class="rounded bg-white/5 px-1">Cmd+Tab</code>.
        </li>
        <li>
          Prefer
          <code class="rounded bg-white/5 px-1">Ctrl+Shift+Space</code>
          or
          <code class="rounded bg-white/5 px-1">Cmd+Shift+D</code>.
        </li>
        <li>
          Use the menu bar icon → <strong>Start Listening</strong> as a fallback (works without a
          global grab).
        </li>
      </ul>
    </div>
  </div>
</section>
