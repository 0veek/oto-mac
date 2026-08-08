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

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Stats</h2>
    <p class="section__lead">
      Counted from the history on this machine. Nothing is uploaded and nothing is
      tracked separately — turning history off under Privacy empties this page too.
    </p>
  </header>

  {#if error}
    <p class="note note--bad">Could not read history ({error}).</p>
  {:else if !stats}
    <p class="loading">Reading history…</p>
  {:else if stats.total_sessions === 0}
    <p class="empty">Nothing dictated yet. Hold your shortcut and say something.</p>
  {:else}
    <div class="tiles">
      {#each [
        { label: "Words", value: numberFormat.format(stats.total_words), unit: "total" },
        { label: "Dictations", value: numberFormat.format(stats.total_sessions), unit: "" },
        { label: "Today", value: numberFormat.format(stats.words_today), unit: "words" },
        {
          label: "Typical length",
          value: String(Math.round(stats.average_words_per_session)),
          unit: "words",
        },
      ] as tile (tile.label)}
        <div class="tile">
          <span class="plate-micro tile__label">{tile.label}</span>
          <span class="tile__value">
            {tile.value}
            {#if tile.unit}<span class="tile__unit">{tile.unit}</span>{/if}
          </span>
        </div>
      {/each}
    </div>

    <div class="tiles">
      <div class="tile">
        <span class="plate-micro tile__label">Time saved</span>
        <span class="tile__value">{formatMinutes(stats.estimated_minutes_saved)}</span>
        <p class="tile__note">
          Against typing the same words at 40 wpm, speaking at 150. An estimate, not a
          measurement.
        </p>
      </div>

      <div class="tile">
        <span class="plate-micro tile__label">Streak</span>
        <span class="tile__value">
          {#if stats.current_streak_days > 0}
            <span class="tile__flame"><IconFlame aria-hidden="true" size={18} stroke={1.9} /></span>
          {/if}
          {stats.current_streak_days}
          <span class="tile__unit">{stats.current_streak_days === 1 ? "day" : "days"}</span>
        </span>
        <p class="tile__note">Your best run so far is {stats.best_streak_days} days.</p>
      </div>
    </div>

    <div class="rack">
      <div class="rack__head">
        <span class="plate-micro rack__title">Last 30 days</span>
      </div>
      <div class="chart">
        <div
          class="bars"
          role="img"
          aria-label="Words dictated per day over the last 30 days"
        >
          {#each series as day (day.days_ago)}
            <div
              class="bars__bar"
              data-empty={day.words === 0}
              data-today={day.days_ago === 0}
              style="height: {day.words > 0 ? Math.max(4, (day.words / peak) * 100) : 2}%"
              title="{day.days_ago === 0
                ? 'Today'
                : `${day.days_ago} day${day.days_ago === 1 ? '' : 's'} ago`}: {day.words} words"
            ></div>
          {/each}
        </div>
        <div class="plate-micro bars__scale">
          <span>30 days ago</span>
          <span>{numberFormat.format(peak)} words peak</span>
          <span>Today</span>
        </div>
      </div>
    </div>
  {/if}
</section>

<style>
  .chart {
    padding-block-start: var(--space-md);
  }

  .tile__flame {
    display: inline-flex;
    color: var(--lamp);
  }
</style>
