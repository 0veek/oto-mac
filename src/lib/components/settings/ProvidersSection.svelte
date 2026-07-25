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

  async function refreshKeyInfo(preset: ProviderPreset) {
    try {
      if (preset === "custom" && config.active_custom_provider_id) {
        const present = await invoke<boolean>("provider_api_key_present", {
          account: `custom:${config.active_custom_provider_id}`,
        });
        keyPresent = present;
        keyHint = present ? "••••" : null;
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
    keyDraft = "";
    keyStatus = null;
  }

  async function saveKey() {
    keyBusy = true;
    keyStatus = null;
    try {
      if (config.provider_preset === "custom" && config.active_custom_provider_id) {
        await invoke("set_provider_api_key", {
          account: `custom:${config.active_custom_provider_id}`,
          key: keyDraft,
        });
      } else {
        await invoke("set_api_key", {
          preset: config.provider_preset,
          key: keyDraft,
        });
      }
      keyDraft = "";
      await refreshKeyInfo(config.provider_preset);
      keyStatus = keyPresent ? "API key saved to keyring" : "API key cleared";
    } catch (e) {
      keyStatus = `Failed to save key: ${String(e)}`;
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

<section class="space-y-6">
  <header>
    <h2 class="text-xl font-semibold tracking-tight">Providers</h2>
    <p class="mt-1 text-sm text-slate-400">
      Choose a cloud STT provider and store your API key in the OS keyring. OpenAI, Groq, and OpenRouter also power optional LLM polish; Deepgram is speech-to-text only (Nova-3).
    </p>
  </header>

  <div class="settings-panel provider-panel">
    <label class="provider-row">
      <span class="provider-row__label">Provider preset</span>
      <div class="provider-row__control select-wrap">
        <select class="provider-select" value={config.provider_preset} onchange={onPresetChange}>
          {#each PRESET_OPTIONS as opt}
            <option value={opt.value}>{opt.label}</option>
          {/each}
        </select>
        <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
      </div>
    </label>

    {#if config.provider_preset === "custom"}
      <div class="provider-custom space-y-3">
        <div class="flex items-center justify-between gap-3">
          <div><div class="text-sm font-medium text-slate-200">Provider profiles</div><div class="text-xs text-slate-500">Declarative plugins for OpenAI-compatible endpoints.</div></div>
          <button type="button" class="rounded-lg bg-white/10 px-3 py-1.5 text-xs hover:bg-white/15" onclick={addProfile}>Add profile</button>
        </div>
        <div class="select-wrap">
          <select class="w-full rounded-lg border border-white/10 bg-slate-950 px-3 py-2 text-sm text-white" value={config.active_custom_provider_id ?? ""} onchange={(event) => config.active_custom_provider_id = event.currentTarget.value || null}>
            <option value="">Legacy custom fields below</option>
            {#each config.custom_providers as profile (profile.id)}<option value={profile.id}>{profile.name}</option>{/each}
          </select>
          <IconChevronDown aria-hidden="true" size={16} stroke={1.7} />
        </div>
        {#if config.active_custom_provider_id}
          {@const profile = config.custom_providers.find((item) => item.id === config.active_custom_provider_id)}
          {#if profile}
            <div class="grid gap-2 sm:grid-cols-2">
              <input aria-label="Profile name" class="rounded-lg border border-white/10 bg-slate-950 px-3 py-2 text-sm" value={profile.name} oninput={(event) => patchProfile(profile.id, { name: event.currentTarget.value })} />
              <input aria-label="Profile base URL" class="rounded-lg border border-white/10 bg-slate-950 px-3 py-2 text-sm" value={profile.base_url} oninput={(event) => patchProfile(profile.id, { base_url: event.currentTarget.value })} />
              <input aria-label="Profile STT model" class="rounded-lg border border-white/10 bg-slate-950 px-3 py-2 text-sm" value={profile.stt_model} oninput={(event) => patchProfile(profile.id, { stt_model: event.currentTarget.value })} />
              <input aria-label="Profile polish model" class="rounded-lg border border-white/10 bg-slate-950 px-3 py-2 text-sm" value={profile.polish_model} oninput={(event) => patchProfile(profile.id, { polish_model: event.currentTarget.value })} />
            </div>
            <button type="button" class="text-xs text-rose-300" onclick={() => removeProfile(profile.id)}>Remove this profile</button>
          {:else}
            <p class="text-xs text-amber-300">
              Active profile is missing. Choose another profile or use legacy fields below.
            </p>
          {/if}
        {/if}
      </div>
    {/if}

    {#if config.provider_preset !== "custom" || !config.active_custom_provider_id}
      <label class="provider-row">
        <span class="provider-row__label">Base URL</span>
        <span class="provider-row__control">
          <input
            type="url"
            placeholder="https://api.example.com/v1"
            bind:value={config.base_url}
          />
          <small>
            {#if config.provider_preset === "deepgram"}
              Deepgram API root (https://api.deepgram.com). Auth uses Token header, not Bearer.
            {:else}
              OpenAI-compatible API root (…/v1). Updated automatically for known presets.
            {/if}
          </small>
        </span>
      </label>
    {:else}
      <p class="provider-profile-note">
        Base URL and model IDs come from the active provider profile above.
      </p>
    {/if}

    <div class="provider-row">
      <span class="provider-row__label">API key</span>
      <div class="provider-row__control">
        <div class="provider-key-control">
          <input
            type="password"
            placeholder={keyPresent ? "Enter new key to replace…" : "sk-…"}
            autocomplete="off"
            spellcheck="false"
            bind:value={keyDraft}
          />
          <button type="button" disabled={keyBusy} onclick={saveKey}>
            {keyBusy ? "Saving…" : "Save Key"}
          </button>
        </div>
        <small>
          Keys never write to config.json — only the OS keyring.
          {#if keyPresent && keyHint}
            <span class="text-emerald-400/90"> Stored: {keyHint}</span>
          {:else if !keyPresent}
            <span class="text-amber-400/90"> No key stored for this preset.</span>
          {/if}
        </small>
        {#if keyStatus}
          <small class="provider-key-status">{keyStatus}</small>
        {/if}
      </div>
    </div>
  </div>
</section>
