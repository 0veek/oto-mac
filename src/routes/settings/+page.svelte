<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { IconCircleCheck } from "@tabler/icons-svelte";
  import type { AppConfig, PipelineEvent } from "$lib/types";
  import { applyPipelineEvent } from "$lib/stores/pipeline";
  import SettingsShell from "$lib/components/settings/SettingsShell.svelte";
  import ProvidersSection from "$lib/components/settings/ProvidersSection.svelte";
  import ModelsSection from "$lib/components/settings/ModelsSection.svelte";
  import HotkeysSection from "$lib/components/settings/HotkeysSection.svelte";
  import AudioSection from "$lib/components/settings/AudioSection.svelte";
  import ModesSection from "$lib/components/settings/ModesSection.svelte";
  import DictionarySection from "$lib/components/settings/DictionarySection.svelte";
  import SnippetsSection from "$lib/components/settings/SnippetsSection.svelte";
  import StylesSection from "$lib/components/settings/StylesSection.svelte";
  import HistorySection from "$lib/components/settings/HistorySection.svelte";
  import StatsSection from "$lib/components/settings/StatsSection.svelte";
  import PrivacySection from "$lib/components/settings/PrivacySection.svelte";
  import AppearanceSection from "$lib/components/settings/AppearanceSection.svelte";
  import InjectionSection from "$lib/components/settings/InjectionSection.svelte";
  import PermissionsSection from "$lib/components/settings/PermissionsSection.svelte";
  import AboutSection from "$lib/components/settings/AboutSection.svelte";
  import OnboardingWizard from "$lib/components/OnboardingWizard.svelte";
  import { applyTheme } from "$lib/theme";

  const SECTIONS = [
    { id: "providers", label: "Providers" },
    { id: "models", label: "Models" },
    { id: "hotkeys", label: "Hotkeys" },
    { id: "audio", label: "Audio" },
    { id: "modes", label: "Modes" },
    { id: "dictionary", label: "Dictionary" },
    { id: "snippets", label: "Snippets" },
    { id: "styles", label: "Styles & commands" },
    { id: "history", label: "History" },
    { id: "stats", label: "Stats" },
    { id: "permissions", label: "Permissions" },
    { id: "appearance", label: "Appearance" },
    { id: "privacy", label: "Privacy" },
    { id: "injection", label: "Injection" },
    { id: "about", label: "About" },
  ] as const;

  type SectionId = (typeof SECTIONS)[number]["id"];

  let config = $state<AppConfig | null>(null);
  let active = $state<SectionId>("providers");
  let loadError = $state<string | null>(null);
  let saveStatus = $state<string | null>(null);
  let saving = $state(false);
  /** Serialized copy of the last config the backend confirmed, for dirty tracking. */
  let savedSnapshot = $state<string | null>(null);

  const dirty = $derived(
    config !== null && savedSnapshot !== null && JSON.stringify(config) !== savedSnapshot,
  );

  const SAVABLE: SectionId[] = [
    "providers",
    "models",
    "hotkeys",
    "audio",
    "modes",
    "dictionary",
    "snippets",
    "styles",
    "appearance",
    "privacy",
    "injection",
  ];

  function browserPreviewDefaults(): AppConfig {
    return {
      provider_preset: "groq",
      base_url: "https://api.groq.com/openai/v1",
      stt_model: "whisper-large-v3",
      polish_model: "llama-3.1-8b-instant",
      polish_enabled: true,
      temperature: 0.2,
      tone_hint: "",
      hotkey: "Ctrl+Super+Space",
      language: null,
      dictionary: [],
      injection_mode: "auto",
      idle_behavior: "hide",
      overlay_x: null,
      overlay_y: null,
      stt_backend: "cloud",
      local_whisper_model_path: "",
      vocabulary_boost: true,
      snippets: [],
      styles: [
        { id: "professional", name: "Professional", prompt: "Professional, clear, and concise." },
        { id: "casual", name: "Casual", prompt: "Natural and friendly." },
      ],
      active_style_id: null,
      history_enabled: true,
      history_limit: 100,
      streaming_enabled: false,
      theme: "midnight",
      reduce_motion: false,
      font_scale: 1,
      custom_providers: [],
      active_custom_provider_id: null,
      sync: { enabled: false, endpoint: "" },
      // No `autostart_enabled` here: this port drives launch-at-login through
      // tauri-plugin-autostart, so it is not part of AppConfig at all.
      activation_mode: "hold",
      hybrid_tap_threshold_ms: 350,
      audio: {
        input_device: null,
        input_gain: 1,
        noise_gate: false,
        noise_gate_threshold: 0.02,
      },
      vad: { auto_stop: true, silence_ms: 1500, min_speech_ms: 400 },
      sounds: {
        enabled: false,
        volume: 0.4,
        on_start: true,
        on_stop: true,
        on_done: false,
        on_error: true,
      },
      modes: [],
      voice_edits_enabled: true,
      replacements: [],
      keep_history_audio: false,
      onboarding_complete: true,
      context_level: "app",
      context_blocklist: [],
    };
  }

  /** True only outside a Tauri webview. `tauri dev` also serves over http://. */
  function isBrowserPreview() {
    return !("__TAURI_INTERNALS__" in window);
  }

  async function loadConfig() {
    loadError = null;
    try {
      config = await invoke<AppConfig>("get_config");
      savedSnapshot = JSON.stringify(config);
    } catch (e) {
      // Browser without Tauri: keep the page usable for layout checks only.
      // Never install defaults into a live Tauri session — Save would wipe disk config.
      if (isBrowserPreview()) {
        loadError = null;
        config = browserPreviewDefaults();
        savedSnapshot = JSON.stringify(config);
      } else {
        loadError = String(e);
        config = null;
        savedSnapshot = null;
      }
    }
  }

  // This backend has no `config://changed` broadcast, so the snapshot can only be
  // refreshed by asking. Coming back to the front is the moment that matters: the
  // pill writes its own coordinates when dragged, and adopting them here stops the
  // stale draft from moving it back on the next save. Reading disk never touches
  // the user's edits, so a genuinely dirty form stays dirty.
  async function resyncSavedSnapshot() {
    if (!config || saving) return;
    try {
      const latest = await invoke<AppConfig>("get_config");
      config.overlay_x = latest.overlay_x;
      config.overlay_y = latest.overlay_y;
      savedSnapshot = JSON.stringify(latest);
    } catch {
      // Keep the previous snapshot; the form stays usable either way.
    }
  }

  async function saveConfig() {
    if (!config || saving || loadError) return;
    saving = true;
    saveStatus = null;
    try {
      // Normalize values the backend also clamps so the form stays consistent.
      config.history_limit = Math.min(1000, Math.max(1, Math.round(Number(config.history_limit) || 100)));
      config.font_scale = Math.min(1.25, Math.max(0.85, Number(config.font_scale) || 1));
      config.temperature = Math.min(1, Math.max(0, Number(config.temperature) || 0));
      // Never pass API keys through set_config — keys use set_api_key only
      await invoke("set_config", { cfg: config });
      // Reload so server-side normalization (hotkey formatting, etc.) is reflected.
      try {
        config = await invoke<AppConfig>("get_config");
        savedSnapshot = JSON.stringify(config);
      } catch {
        // Keep local draft if reload fails.
      }
      saveStatus = "Saved";
      setTimeout(() => {
        if (saveStatus === "Saved") saveStatus = null;
      }, 2000);
    } catch (e) {
      const message = String(e);
      // A rejected chord is the one failure the backend recovers from: it keeps
      // the last shortcut that bound and still persists every other change.
      // Reload so the form shows exactly what is now on disk.
      const hotkeyFailure = ["hotkey", "shortcut"].some((term) =>
        message.toLowerCase().includes(term),
      );
      try {
        config = await invoke<AppConfig>("get_config");
        savedSnapshot = JSON.stringify(config);
        saveStatus = hotkeyFailure
          ? `Hotkey not registered: ${message} Your other changes were saved and the previous shortcut is still active.`
          : `Save failed: ${message}. Reloaded last saved settings.`;
      } catch {
        saveStatus = `Save failed: ${message}`;
      }
    } finally {
      saving = false;
    }
  }

  onMount(() => {
    const requestedSection = new URLSearchParams(window.location.search).get("section");
    if (requestedSection && SECTIONS.some((section) => section.id === requestedSection)) {
      active = requestedSection as SectionId;
    }
    void loadConfig();
    const onKeydown = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s" && SAVABLE.includes(active)) {
        event.preventDefault();
        void saveConfig();
      }
    };
    window.addEventListener("keydown", onKeydown);

    const onFocus = () => void resyncSavedSnapshot();
    window.addEventListener("focus", onFocus);

    // The rail's input meter is wired to the same broadcast the overlay uses, so
    // the settings window shows real levels while you dictate or run a mic test.
    // `.catch` is attached here, not in the teardown: outside a Tauri webview
    // `listen` rejects immediately and would surface as an unhandled rejection.
    const unlistenPipeline = listen<PipelineEvent>("pipeline://event", ({ payload }) => {
      applyPipelineEvent(payload);
    }).catch(() => null);

    return () => {
      window.removeEventListener("keydown", onKeydown);
      window.removeEventListener("focus", onFocus);
      void unlistenPipeline.then((unlisten) => unlisten?.());
    };
  });

  $effect(() => {
    if (config) applyTheme(config.theme, config.reduce_motion, config.font_scale);
  });
</script>

{#if config && !config.onboarding_complete}
  <!-- First run only: an existing config.json always arrives with this set. -->
  <div class="oto-app-plain" data-theme={config.theme}>
    <!-- No rail here to carry the traffic lights, so the reserved strip is the
         window's only drag handle. -->
    <div class="mac-drag" data-tauri-drag-region></div>
    <OnboardingWizard
      bind:config
      ondone={() => {
        savedSnapshot = JSON.stringify(config);
        saveStatus = null;
      }}
    />
  </div>
{:else if !config}
  <div class="oto-app-plain boot">
    <div class="mac-drag" data-tauri-drag-region></div>
    {#if loadError}
      <p class="plate-micro boot__label">No link to Oto</p>
      <p class="boot__body">
        Settings could not be read ({loadError}). Nothing on disk was changed.
      </p>
      <button type="button" class="btn" onclick={() => void loadConfig()}>Try again</button>
    {:else}
      <p class="plate-micro boot__label">Reading settings</p>
    {/if}
  </div>
{:else}
  <SettingsShell
    sections={[...SECTIONS]}
    {active}
    theme={config.theme}
    onselect={(id) => {
      active = id as SectionId;
      saveStatus = null;
    }}
  >
    <div class="stage__inner">

      {#if active === "providers"}
        <ProvidersSection bind:config />
      {:else if active === "models"}
        <ModelsSection bind:config />
      {:else if active === "hotkeys"}
        <HotkeysSection bind:config />
      {:else if active === "audio"}
        <AudioSection bind:config />
      {:else if active === "modes"}
        <ModesSection bind:config />
      {:else if active === "dictionary"}
        <DictionarySection bind:config />
      {:else if active === "snippets"}
        <SnippetsSection bind:config />
      {:else if active === "styles"}
        <StylesSection bind:config />
      {:else if active === "history"}
        <HistorySection />
      {:else if active === "stats"}
        <StatsSection />
      {:else if active === "appearance"}
        <AppearanceSection bind:config />
      {:else if active === "permissions"}
        <PermissionsSection
          onselect={(id) => {
            active = id as SectionId;
            saveStatus = null;
          }}
        />
      {:else if active === "privacy"}
        <PrivacySection bind:config />
      {:else if active === "injection"}
        <InjectionSection bind:config />
      {:else if active === "about"}
        <AboutSection />
      {/if}

      {#if SAVABLE.includes(active)}
        <div class="actionbar">
          {#if dirty}
            <span class="actionbar__note">Unsaved changes — ⌘S applies them.</span>
          {/if}
          {#if saveStatus && saveStatus !== "Saved"}
            <span class="actionbar__status" role="alert">{saveStatus}</span>
          {/if}
          <button
            type="button"
            class="btn btn--primary actionbar__save"
            data-dirty={dirty}
            disabled={saving}
            onclick={saveConfig}
          >
            {#if saveStatus === "Saved"}
              <IconCircleCheck aria-hidden="true" size={16} stroke={1.8} />
              Saved
            {:else}
              {saving ? "Saving…" : "Save changes"}
            {/if}
          </button>
        </div>
      {/if}
    </div>
  </SettingsShell>
{/if}

<style>
  .boot {
    display: grid;
    align-content: center;
    justify-items: center;
    gap: var(--space-sm);
    min-height: 100dvh;
    padding: var(--space-xl);
    text-align: center;
  }

  .boot__label {
    color: var(--faint);
  }

  .boot__body {
    max-width: 34rem;
    color: var(--muted);
    font-size: var(--text-sm);
    line-height: 1.55;
  }

  /* Sits exactly in the strip `.oto-app-plain` already reserves for the traffic
     lights, so it covers no content while giving the title-bar-less window
     something to be dragged by. Painted in the chassis colour because the strip
     is padding, not a clip: without a fill, scrolled content would ride up
     behind the traffic lights. */
  .mac-drag {
    position: fixed;
    inset-block-start: 0;
    inset-inline: 0;
    z-index: var(--z-sticky);
    height: var(--mac-titlebar);
    background: var(--chassis);
  }
</style>
