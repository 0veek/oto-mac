<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { IconChevronDown } from "@tabler/icons-svelte";
  import type { AppConfig, ProviderPreset, ProviderProfile } from "$lib/types";

  const PRESET_DEFAULTS: Record<
    Exclude<ProviderPreset, "custom">,
    { base_url: string; stt_model: string; polish_model: string }
  > = {
    open_ai: {
      base_url: "https://api.openai.com/v1",
      stt_model: "whisper-1",
      polish_model: "gpt-4o-mini",
    },
    groq: {
      base_url: "https://api.groq.com/openai/v1",
      stt_model: "whisper-large-v3",
      polish_model: "llama-3.1-8b-instant",
    },
    open_router: {
      base_url: "https://openrouter.ai/api/v1",
      stt_model: "openai/whisper-1",
      polish_model: "openai/gpt-4o-mini",
    },
    deepgram: {
      base_url: "https://api.deepgram.com",
      stt_model: "nova-3",
      // Deepgram is STT-only; polish needs a separate OpenAI-compatible LLM.
      polish_model: "",
    },
  };

  const PRESET_OPTIONS: { value: ProviderPreset; label: string }[] = [
    { value: "open_ai", label: "OpenAI" },
    { value: "groq", label: "Groq" },
    { value: "open_router", label: "OpenRouter" },
    { value: "deepgram", label: "Deepgram" },
    { value: "custom", label: "Custom" },
  ];

  let {
    config = $bindable(),
  }: {
    config: AppConfig;
  } = $props();

  let keyDraft = $state("");
  let keyHint = $state<string | null>(null);
  let keyPresent = $state(false);
  let keyStatus = $state<string | null>(null);
  let keyBusy = $state(false);

  /** Resolve the Keychain account for the active provider (avoids `custom:null`). */
  function customKeyAccount(): string | null {
    const id = config.active_custom_provider_id;
    if (typeof id === "string" && id.length > 0 && id !== "null") {
      return `custom:${id}`;
    }
    return null;
  }

  async function refreshKeyInfo(preset: ProviderPreset) {
    try {
      if (preset === "custom") {
        const account = customKeyAccount();
        if (account) {
          const present = await invoke<boolean>("provider_api_key_present", { account });
          if (present) {
            keyPresent = true;
            keyHint = "••••";
            return;
          }
          // Fall back to legacy account "custom" for older installs.
        }
        const [present, hint] = await Promise.all([
          invoke<boolean>("api_key_present", { preset: "custom" }),
          invoke<string | null>("api_key_hint", { preset: "custom" }),
        ]);
        keyPresent = present;
        keyHint = hint;
        return;
      }
      const [present, hint] = await Promise.all([
        invoke<boolean>("api_key_present", { preset }),
        invoke<string | null>("api_key_hint", { preset }),
      ]);
      keyPresent = present;
      keyHint = hint;
    } catch {
      keyPresent = false;
      keyHint = null;
    }
  }

  $effect(() => {
    config.active_custom_provider_id;
    void refreshKeyInfo(config.provider_preset);
  });

  function onPresetChange(event: Event) {
    const value = (event.target as HTMLSelectElement).value as ProviderPreset;
    const leavingDeepgram = config.provider_preset === "deepgram" && value !== "deepgram";
    config.provider_preset = value;
    if (value !== "custom") {
      const defaults = PRESET_DEFAULTS[value];
      config.base_url = defaults.base_url;
      config.stt_model = defaults.stt_model;
      config.polish_model = defaults.polish_model;
      // Deepgram smart_format handles punctuation; LLM polish is not available on this API.
      if (value === "deepgram") {
        config.polish_enabled = false;
      }
    }
    // Undo the forced-off toggle when moving back to a provider that can polish,
    // otherwise polish stays silently disabled long after Deepgram is gone.
    if (leavingDeepgram) {
      config.polish_enabled = true;
    }
    keyDraft = "";
    keyStatus = null;
  }

  /** Write `key` to the Keychain for the active provider ("" removes it). */
  async function writeKey(key: string) {
    const customAccount = config.provider_preset === "custom" ? customKeyAccount() : null;
    if (customAccount) {
      await invoke("set_provider_api_key", { account: customAccount, key });
    } else {
      await invoke("set_api_key", { preset: config.provider_preset, key });
    }
  }

  async function saveKey() {
    // An empty field used to silently delete the stored key. Deleting is now an
    // explicit action (Remove) so a stray click cannot revoke a working setup.
    if (!keyDraft.trim()) {
      keyStatus = "Enter a key first, or use Remove to delete the stored one.";
      return;
    }
    keyBusy = true;
    keyStatus = null;
    try {
      await writeKey(keyDraft.trim());
      keyDraft = "";
      await refreshKeyInfo(config.provider_preset);
      keyStatus = "API key saved to the Keychain";
    } catch (e) {
      keyStatus = `Failed to save key: ${String(e)}`;
    } finally {
      keyBusy = false;
    }
  }

  async function removeKey() {
    const label = PRESET_OPTIONS.find((option) => option.value === config.provider_preset)?.label
      ?? config.provider_preset;
    if (!confirm(`Delete the stored ${label} API key from the macOS Keychain?`)) return;
    keyBusy = true;
    keyStatus = null;
    try {
      await writeKey("");
      keyDraft = "";
      await refreshKeyInfo(config.provider_preset);
      keyStatus = "API key removed from the Keychain";
    } catch (e) {
      keyStatus = `Failed to remove key: ${String(e)}`;
    } finally {
      keyBusy = false;
    }
  }

  function addProfile() {
    const id = globalThis.crypto?.randomUUID?.() ?? `provider-${Date.now()}`;
    config.custom_providers = [...config.custom_providers, {
      id,
      name: "New provider",
      base_url: "https://api.example.com/v1",
      stt_model: "whisper-1",
      polish_model: "gpt-4o-mini",
    }];
    config.provider_preset = "custom";
    config.active_custom_provider_id = id;
  }

  function patchProfile(id: string, patch: Partial<ProviderProfile>) {
    config.custom_providers = config.custom_providers.map((profile) => profile.id === id ? { ...profile, ...patch } : profile);
  }

  function removeProfile(id: string) {
    config.custom_providers = config.custom_providers.filter((profile) => profile.id !== id);
    if (config.active_custom_provider_id === id) {
      config.active_custom_provider_id = config.custom_providers[0]?.id ?? null;
    }
  }
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Providers</h2>
    <p class="section__lead">
      Where speech becomes text. OpenAI, Groq, and OpenRouter also run the optional
      cleanup pass; Deepgram transcribes only. Your key goes to the macOS Keychain,
      never to config.json.
    </p>
  </header>

  <div class="rack">
    <label class="row">
      <span class="row__label">Provider</span>
      <span class="row__control select-wrap">
        <select value={config.provider_preset} onchange={onPresetChange}>
          {#each PRESET_OPTIONS as opt (opt.value)}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
        <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
      </span>
    </label>

    {#if config.provider_preset === "custom"}
      <div class="row row--stacked">
        <span class="row__label">Profiles</span>
        <div class="row__control">
          <div class="item">
            <div class="item__head">
              <span class="item__title">Endpoint profiles</span>
              <button type="button" class="btn btn--small" onclick={addProfile}>Add profile</button>
            </div>
            <div class="select-wrap">
              <select
                aria-label="Active profile"
                value={config.active_custom_provider_id ?? ""}
                onchange={(event) =>
                  (config.active_custom_provider_id = event.currentTarget.value || null)}
              >
                <option value="">Use the fields below</option>
                {#each config.custom_providers as profile (profile.id)}
                  <option value={profile.id}>{profile.name}</option>
                {/each}
              </select>
              <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
            </div>

            {#if config.active_custom_provider_id}
              {@const profile = config.custom_providers.find(
                (item) => item.id === config.active_custom_provider_id,
              )}
              {#if profile}
                <div class="item__grid item__grid--pair">
                  <input
                    aria-label="Profile name"
                    value={profile.name}
                    oninput={(event) => patchProfile(profile.id, { name: event.currentTarget.value })}
                  />
                  <input
                    aria-label="Profile base URL"
                    class="field-data"
                    value={profile.base_url}
                    oninput={(event) =>
                      patchProfile(profile.id, { base_url: event.currentTarget.value })}
                  />
                  <input
                    aria-label="Profile speech model"
                    class="field-data"
                    value={profile.stt_model}
                    oninput={(event) =>
                      patchProfile(profile.id, { stt_model: event.currentTarget.value })}
                  />
                  <input
                    aria-label="Profile cleanup model"
                    class="field-data"
                    value={profile.polish_model}
                    oninput={(event) =>
                      patchProfile(profile.id, { polish_model: event.currentTarget.value })}
                  />
                </div>
                <button
                  type="button"
                  class="btn-link btn-link--danger"
                  onclick={() => removeProfile(profile.id)}
                >
                  Remove this profile
                </button>
              {:else}
                <p class="note note--warn">
                  This profile no longer exists. Pick another, or clear the selection to use the
                  fields below.
                </p>
              {/if}
            {/if}
          </div>
        </div>
      </div>
    {/if}

    {#if config.provider_preset !== "custom" || !config.active_custom_provider_id}
      <label class="row">
        <span class="row__label">Base URL</span>
        <span class="row__control">
          <input
            type="url"
            class="field-data"
            placeholder="https://api.example.com/v1"
            bind:value={config.base_url}
          />
          <span class="row__hint">
            {#if config.provider_preset === "deepgram"}
              Deepgram API root. Authentication uses a Token header rather than Bearer.
            {:else}
              OpenAI-compatible API root, ending in /v1. Known providers fill this in for you.
            {/if}
          </span>
        </span>
      </label>
    {:else}
      <div class="row">
        <span class="row__label">Base URL</span>
        <p class="row__control row__hint">
          Set by the selected profile, along with both model IDs.
        </p>
      </div>
    {/if}

    <div class="row row--flush">
      <span class="row__label">API key</span>
      <div class="row__control">
        <div class="btn-row key-field">
          <input
            type="password"
            class="field-data key-field__input"
            placeholder={keyPresent ? "Enter a new key to replace it" : "Paste your key"}
            autocomplete="off"
            spellcheck="false"
            aria-label="API key"
            bind:value={keyDraft}
          />
          <button type="button" class="btn" disabled={keyBusy || !keyDraft.trim()} onclick={saveKey}>
            {keyBusy ? "Saving…" : "Save key"}
          </button>
          {#if keyPresent}
            <button type="button" class="btn btn--danger" disabled={keyBusy} onclick={removeKey}>
              Remove
            </button>
          {/if}
        </div>

        {#if keyPresent && keyHint}
          <p class="row__hint">
            In the Keychain as <span class="readout status-ok">{keyHint}</span>.
          </p>
        {:else}
          <p class="row__hint status-warn">
            No key stored for this provider — dictation will fail until you add one.
          </p>
        {/if}

        {#if keyStatus}
          <p class="row__hint">{keyStatus}</p>
        {/if}
      </div>
    </div>
  </div>
</section>

<style>
  .key-field {
    flex-wrap: nowrap;
  }

  .key-field__input {
    min-width: 0;
    flex: 1;
  }

  @media (max-width: 30rem) {
    .key-field {
      flex-wrap: wrap;
    }

    .key-field__input {
      flex-basis: 100%;
    }
  }
</style>
