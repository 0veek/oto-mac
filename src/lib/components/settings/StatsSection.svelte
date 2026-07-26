<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { IconFlame } from "@tabler/icons-svelte";
  import type { UsageStats } from "$lib/types";

  let stats = $state<UsageStats | null>(null);
  let error = $state<string | null>(null);

  async function refresh() {
    error = null;
    try {
      stats = await invoke<UsageStats>("get_usage_stats");
    } catch (e) {
      stats = null;
      error = String(e);
    }
  }

  function formatMinutes(minutes: number) {
    if (minutes < 1) return "under a minute";
    if (minutes < 60) return `${Math.round(minutes)} min`;
    const hours = Math.floor(minutes / 60);
    const rest = Math.round(minutes % 60);
    return rest ? `${hours} h ${rest} min` : `${hours} h`;
  }

  const numberFormat = new Intl.NumberFormat();

  // The chart reads oldest-to-newest left-to-right, which is how people expect
  // to see time move.
  let series = $derived(stats ? [...stats.daily].reverse() : []);
  let peak = $derived(Math.max(1, ...series.map((day) => day.words)));

  onMount(() => {
    void refresh();
  });
</script>

<section class="space-y-6">
  <header>
    <h2 class="text-xl font-semibold tracking-tight">Stats</h2>
    <p class="mt-1 text-sm text-slate-400">
      Computed from your local history — nothing is uploaded, and nothing is tracked separately.
      Turning history off under Privacy turns these off too.
    </p>
  </header>

  {#if error}
    <p class="rounded-xl border border-rose-400/25 bg-rose-400/5 px-4 py-3 text-sm text-rose-200">
      Could not read history ({error}).
    </p>
  {:else if !stats}
    <p class="text-sm text-slate-500">Loading…</p>
  {:else if stats.total_sessions === 0}
    <div class="rounded-2xl border border-dashed border-white/15 px-6 py-14 text-center text-sm text-slate-500">
      No dictations recorded yet.
    </div>
  {:else}
    <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
      {#each [
        { label: "Words dictated", value: numberFormat.format(stats.total_words) },
        { label: "Dictations", value: numberFormat.format(stats.total_sessions) },
        { label: "Words today", value: numberFormat.format(stats.words_today) },
        {
          label: "Average length",
          value: `${Math.round(stats.average_words_per_session)} words`,
        },
      ] as tile (tile.label)}
        <div class="rounded-2xl border border-white/10 bg-white/[0.04] p-5 shadow-xl backdrop-blur-xl">
          <p class="text-xs font-medium uppercase tracking-wide text-slate-500">{tile.label}</p>
          <p class="mt-1.5 text-2xl font-semibold tabular-nums tracking-tight text-slate-100">
            {tile.value}
          </p>
        </div>
      {/each}
    </div>

    <div class="grid gap-3 sm:grid-cols-2">
      <div class="rounded-2xl border border-white/10 bg-white/[0.04] p-5 shadow-xl backdrop-blur-xl">
        <p class="text-xs font-medium uppercase tracking-wide text-slate-500">Time saved</p>
        <p class="mt-1.5 text-2xl font-semibold tabular-nums tracking-tight text-slate-100">
          {formatMinutes(stats.estimated_minutes_saved)}
        </p>
        <p class="mt-2 text-xs leading-relaxed text-slate-500">
          Versus typing the same words at 40 wpm, against 150 wpm of speech. An estimate, not a
          measurement.
        </p>
      </div>

      <div class="rounded-2xl border border-white/10 bg-white/[0.04] p-5 shadow-xl backdrop-blur-xl">
        <p class="text-xs font-medium uppercase tracking-wide text-slate-500">Streak</p>
        <p class="mt-1.5 flex items-baseline gap-2 text-2xl font-semibold tabular-nums tracking-tight text-slate-100">
          {#if stats.current_streak_days > 0}
            <span class="text-amber-300"><IconFlame aria-hidden="true" size={22} stroke={1.8} /></span>
          {/if}
          {stats.current_streak_days}
          <span class="text-sm font-normal text-slate-500">
            {stats.current_streak_days === 1 ? "day" : "days"}
          </span>
        </p>
        <p class="mt-2 text-xs text-slate-500">Best run: {stats.best_streak_days} days.</p>
      </div>
    </div>

    <div class="rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl">
      <h3 class="text-sm font-semibold tracking-tight text-slate-200">Last 30 days</h3>
      <div class="mt-5 flex h-32 items-end gap-1" role="img" aria-label="Words dictated per day over the last 30 days">
        {#each series as day (day.days_ago)}
          <div
            class="min-w-0 flex-1 rounded-t transition-colors"
            class:bg-sky-400={day.words > 0}
            class:bg-white={day.words === 0}
            class:opacity-10={day.words === 0}
            style="height: {day.words > 0 ? Math.max(4, (day.words / peak) * 100) : 3}%"
            title="{day.days_ago === 0
              ? 'Today'
              : `${day.days_ago} day${day.days_ago === 1 ? '' : 's'} ago`}: {day.words} words"
          ></div>
        {/each}
      </div>
      <div class="mt-2 flex justify-between text-xs text-slate-600">
        <span>30 days ago</span>
        <span>Today</span>
      </div>
    </div>
  {/if}
</section>
