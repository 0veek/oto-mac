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
    // Saving an empty field used to delete the stored token without warning.
    if (!token.trim()) {
      status = "Enter a token first, or use Remove to delete the stored one.";
      return;
    }
    status = null;
    try {
      await invoke("set_sync_token", { token: token.trim() });
      tokenPresent = true;
      token = "";
      status = "Sync token saved to the Keychain.";
    } catch (error) {
      status = `Failed to save token: ${String(error)}`;
    }
  }

  async function removeToken() {
    if (!confirm("Delete the stored sync bearer token from the macOS Keychain?")) return;
    status = null;
    try {
      await invoke("set_sync_token", { token: "" });
      tokenPresent = false;
      token = "";
      status = "Sync token removed from the Keychain.";
    } catch (error) {
      status = `Failed to remove token: ${String(error)}`;
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

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Privacy</h2>
    <p class="section__lead">
      What Oto is allowed to say about where you are typing, what it keeps on this
      machine, and the one place it will send anything on your instruction.
    </p>
  </header>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">What the cleanup model is told</span>
      <p class="rack__note">
        Knowing where text is heading lets the model format for it — short lines in a chat client,
        prose in an email, no markdown in a terminal. It is also what leaves your machine, so each
        step up is yours to choose. None of this applies when cleanup is off.
      </p>
    </div>

    <div class="row row--stacked" role="radiogroup" aria-label="Disclose">
      <span class="row__label">Disclose</span>
      <div class="row__control choice-list">
        {#each [
          { value: "none", title: "Nothing", detail: "The model sees only what you said." },
          { value: "app", title: "The application name", detail: "Something like “Slack” or “Terminal”." },
          {
            value: "window",
            title: "Application and window title",
            detail: "Titles often carry file paths, channel names and subject lines.",
          },
          {
            value: "selection",
            title: "Application, title, and nearby text",
            detail:
              "Reads the selection through the macOS Accessibility API. The best results, and the most disclosure.",
          },
        ] as const as option (option.value)}
          <label class="choice" data-active={config.context_level === option.value}>
            <input type="radio" name="context-level" value={option.value} bind:group={config.context_level} />
            <span class="choice__copy">
              <strong>{option.title}</strong>
              <span>{option.detail}</span>
            </span>
          </label>
        {/each}
      </div>
    </div>

    <label class="row row--flush">
      <span class="row__label">Never describe</span>
      <span class="row__control">
        <input
          type="text"
          spellcheck="false"
          placeholder="my-journal, banking-app"
          value={config.context_blocklist.join(", ")}
          onchange={(event) => {
            config.context_blocklist = event.currentTarget.value
              .split(",")
              .map((entry) => entry.trim())
              .filter(Boolean);
          }}
        />
        <span class="row__hint">
          Comma separated, matched as a substring of the application name or bundle id. A blocked
          application discloses nothing at all, not even its name. Password managers, the keychain
          and authenticators are always blocked and cannot be taken off that list. Use
          <strong>Modes → Read focused application</strong> to see exactly what would be sent.
        </span>
      </span>
    </label>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">History</span>
    </div>

    <label class="row row--switch">
      <span class="row__copy">
        <strong>Keep what I dictate</strong>
        <span>Stored under Oto's local application data directory, on this machine only.</span>
      </span>
      <input type="checkbox" bind:checked={config.history_enabled} />
    </label>

    <label class="row row--switch">
      <span class="row__copy">
        <strong>Keep the audio too</strong>
        <span>
          Lets you replay a recording and transcribe it again with different settings. Audio goes
          when its entry goes, whether you delete it or it falls past the limit below.
        </span>
      </span>
      <input
        type="checkbox"
        disabled={!config.history_enabled}
        bind:checked={config.keep_history_audio}
      />
    </label>

    <label class="row row--flush">
      <span class="row__label">Keep at most</span>
      <span class="row__control">
        <input
          type="number"
          min="1"
          max="1000"
          disabled={!config.history_enabled}
          value={config.history_limit}
          oninput={(event) => {
            const next = Number(event.currentTarget.value);
            config.history_limit = Number.isFinite(next)
              ? Math.min(1000, Math.max(1, Math.round(next)))
              : 100;
          }}
        />
        <span class="row__hint">Older dictations drop off once you pass this.</span>
      </span>
    </label>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Sync</span>
      <!-- The merge is local-wins here: `merge_by_id` in src-tauri only appends
           ids it does not already hold, so an edit made elsewhere never
           overwrites this machine's copy. -->
      <p class="rack__note">
        Off until you set it up and press the button yourself. Where two items share an ID your
        local copy wins; anything only on the remote is added. API keys, history, audio and provider
        credentials are never synced.
      </p>
    </div>

    <label class="row row--switch">
      <span class="row__copy">
        <strong>Sync to my own endpoint</strong>
        <span>Merges your dictionary, snippets and styles through a JSON GET and PUT.</span>
      </span>
      <input type="checkbox" bind:checked={config.sync.enabled} />
    </label>

    <label class="row">
      <span class="row__label">Endpoint</span>
      <span class="row__control">
        <input
          type="url"
          class="field-data"
          disabled={!config.sync.enabled}
          placeholder="https://example.com/private/oto.json"
          bind:value={config.sync.endpoint}
        />
      </span>
    </label>

    <div class="row">
      <span class="row__label">Bearer token</span>
      <div class="row__control">
        <div class="btn-row token">
          <input
            type="password"
            class="field-data token__input"
            aria-label="Bearer token"
            disabled={!config.sync.enabled}
            placeholder={tokenPresent ? "Enter a new token to replace it" : "Optional"}
            bind:value={token}
          />
          <button
            type="button"
            class="btn"
            disabled={!config.sync.enabled || !token.trim()}
            onclick={saveToken}
          >
            Save token
          </button>
          {#if tokenPresent}
            <button
              type="button"
              class="btn btn--danger"
              disabled={!config.sync.enabled}
              onclick={removeToken}
            >
              Remove
            </button>
          {/if}
        </div>
      </div>
    </div>

    <div class="row row--switch row--flush">
      <span class="row__copy">
        <strong>Sync now</strong>
        <span>Nothing is sent until you press this.</span>
      </span>
      <button type="button" class="btn" disabled={!config.sync.enabled || busy} onclick={syncNow}>
        {busy ? "Syncing…" : "Sync"}
      </button>
    </div>

    {#if status}
      <p
        aria-live="polite"
        class="note sync-status"
        class:note--bad={status.startsWith("Sync failed") || status.startsWith("Failed")}
        class:note--ok={!status.startsWith("Sync failed") && !status.startsWith("Failed")}
      >
        {status}
      </p>
    {/if}
  </div>
</section>

<style>
  .token {
    flex-wrap: nowrap;
  }

  .token__input {
    min-width: 0;
    flex: 1;
  }

  .sync-status {
    margin-block-start: var(--space-sm);
  }

  @media (max-width: 30rem) {
    .token {
      flex-wrap: wrap;
    }

    .token__input {
      flex-basis: 100%;
    }
  }
</style>
