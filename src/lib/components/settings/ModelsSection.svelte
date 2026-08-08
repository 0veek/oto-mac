<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { IconCloud, IconCpu } from "@tabler/icons-svelte";
  import type { AppConfig, SttBackend } from "$lib/types";
  import PipelineMap from "./PipelineMap.svelte";

  let {
    config = $bindable(),
  }: {
    config: AppConfig;
  } = $props();

  let testBusy = $state(false);
  let testResult = $state<string | null>(null);
  let testError = $state<string | null>(null);

  const activeProfile = $derived(
    config.provider_preset === "custom" && config.active_custom_provider_id
      ? config.custom_providers.find((profile) => profile.id === config.active_custom_provider_id) ?? null
      : null,
  );

  function patchActiveProfile(patch: { stt_model?: string; polish_model?: string }) {
    if (!activeProfile) return;
    config.custom_providers = config.custom_providers.map((profile) =>
      profile.id === activeProfile.id ? { ...profile, ...patch } : profile,
    );
  }

  async function testTranscription() {
    testBusy = true;
    testResult = null;
    testError = null;
    try {
      // Persist current model settings so the test uses what the form shows.
      await invoke("set_config", { cfg: config });
      testResult = await invoke<string>("test_transcription");
    } catch (e) {
      testError = String(e);
    } finally {
      testBusy = false;
    }
  }
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Models</h2>
    <p class="section__lead">
      Which engine turns your voice into text, and what happens to that text before
      it lands in the window you were using.
    </p>
  </header>

  <PipelineMap {config} />

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Transcription</span>
      <p class="rack__note">Where speech becomes text.</p>
    </div>

    <div class="row" role="radiogroup" aria-label="Engine">
      <span class="row__label">Engine</span>
      <div class="row__control">
        <div class="seg">
          <label class="seg__option" data-active={config.stt_backend === "cloud"}>
            <input
              type="radio"
              name="stt_backend"
              value="cloud"
              checked={config.stt_backend === "cloud"}
              onchange={() => (config.stt_backend = "cloud" as SttBackend)}
            />
            <IconCloud aria-hidden="true" size={15} stroke={1.6} />
            Cloud
          </label>
          <label class="seg__option" data-active={config.stt_backend === "local_whisper"}>
            <input
              type="radio"
              name="stt_backend"
              value="local_whisper"
              checked={config.stt_backend === "local_whisper"}
              onchange={() => (config.stt_backend = "local_whisper" as SttBackend)}
            />
            <IconCpu aria-hidden="true" size={15} stroke={1.6} />
            On this machine
          </label>
        </div>
      </div>
    </div>

    {#if config.stt_backend === "local_whisper"}
      <label class="row">
        <span class="row__label">Model file</span>
        <span class="row__control">
          <input
            class="field-data"
            type="text"
            placeholder="/home/you/.local/share/oto/ggml-base.en.bin"
            bind:value={config.local_whisper_model_path}
          />
          <span class="row__hint">
            A whisper.cpp ggml file. Audio never leaves this machine.
          </span>
        </span>
      </label>

      {#if config.polish_enabled}
        <div class="row">
          <span class="row__label"></span>
          <p class="note note--warn row__control">
            Transcription stays local, but cleanup still sends the text to its provider. Turn
            cleanup off below for an entirely offline path.
          </p>
        </div>
      {/if}
    {:else if activeProfile}
      <label class="row">
        <span class="row__label">Speech model</span>
        <span class="row__control">
          <input
            class="field-data"
            type="text"
            placeholder="whisper-large-v3"
            value={activeProfile.stt_model}
            oninput={(event) => patchActiveProfile({ stt_model: event.currentTarget.value })}
          />
          <span class="row__hint">Stored on the “{activeProfile.name}” profile.</span>
        </span>
      </label>
    {:else}
      <label class="row">
        <span class="row__label">Speech model</span>
        <span class="row__control">
          <input
            class="field-data"
            type="text"
            placeholder={config.provider_preset === "deepgram" ? "nova-3" : "whisper-large-v3"}
            bind:value={config.stt_model}
          />
          <span class="row__hint">
            {#if config.provider_preset === "deepgram"}
              A Deepgram model ID — <span class="readout-tight">nova-3</span> by default;
              <span class="readout-tight">nova-3-medical</span> and
              <span class="readout-tight">nova-2</span> also work.
              <span class="readout-tight">smart_format</span> and keyterm prompting are on by
              default.
            {:else}
              Whatever model ID your provider accepts.
            {/if}
          </span>
        </span>
      </label>
    {/if}

    <label class="row">
      <span class="row__label">Language</span>
      <span class="row__control">
        <input
          type="text"
          placeholder="Detect automatically"
          value={config.language ?? ""}
          oninput={(event) => {
            const value = event.currentTarget.value;
            config.language = value.trim() === "" ? null : value.trim();
          }}
        />
        <span class="row__hint">
          An ISO code such as <span class="readout-tight">en</span> or
          <span class="readout-tight">es</span>. Names like “English” work too. Leave it empty to
          detect each time.
        </span>
      </span>
    </label>

    <label class="row row--switch row--flush">
      <span class="row__copy">
        <strong>Bias toward your dictionary</strong>
        <!-- Deepgram takes the terms literally, as Nova-3 keyterm prompts, so the
             copy says so rather than promising a vague nudge. -->
        <span>
          {#if config.provider_preset === "deepgram"}
            Sends dictionary terms as Deepgram keyterm prompts (Nova-3 keyterm prompting).
          {:else}
            Nudges the engine toward the names and terms you have taught it.
          {/if}
        </span>
      </span>
      <input type="checkbox" bind:checked={config.vocabulary_boost} />
    </label>
  </div>

  <div class="rack">
    <div class="rack__head">
      <span class="plate-micro rack__title">Cleanup</span>
      <p class="rack__note">What happens to the transcript before Oto types it.</p>
    </div>

    <label class="row row--switch">
      <span class="row__copy">
        <strong>Show text while you speak</strong>
        <span>
          {#if config.provider_preset === "deepgram" && config.stt_backend === "cloud"}
            Streams to Deepgram as you talk, so the transcript is ready almost the moment you
            stop. Falls back to a single upload if the connection drops.
          {:else if config.stt_backend === "local_whisper"}
            Re-runs Whisper periodically to show partial text.
          {:else}
            Shows partial text where the engine supports it. Today that means Deepgram or local
            Whisper.
          {/if}
        </span>
      </span>
      <input type="checkbox" bind:checked={config.streaming_enabled} />
    </label>

    <label class="row row--switch">
      <span class="row__copy">
        <strong>Clean up the transcript</strong>
        <span>
          {#if config.provider_preset === "deepgram"}
            Needs an OpenAI-compatible model. Deepgram only transcribes, though Nova-3 already
            punctuates.
          {:else}
            Fixes grammar and punctuation, and applies your tone before insertion.
          {/if}
        </span>
      </span>
      <input
        type="checkbox"
        bind:checked={config.polish_enabled}
        disabled={config.provider_preset === "deepgram"}
      />
    </label>

    <label class="row">
      <span class="row__label">Cleanup model</span>
      <span class="row__control">
        {#if activeProfile}
          <input
            class="field-data"
            type="text"
            placeholder="llama-3.1-8b-instant"
            disabled={!config.polish_enabled}
            value={activeProfile.polish_model}
            oninput={(event) => patchActiveProfile({ polish_model: event.currentTarget.value })}
          />
          <span class="row__hint">Stored on the “{activeProfile.name}” profile.</span>
        {:else}
          <input
            class="field-data"
            type="text"
            placeholder="llama-3.1-8b-instant"
            disabled={!config.polish_enabled}
            bind:value={config.polish_model}
          />
        {/if}
      </span>
    </label>

    <label class="row">
      <span class="row__label">Latitude</span>
      <span class="row__control">
        <span class="slider-head">
          <span class="row__hint">Higher values let it rephrase more freely.</span>
          <span class="slider-value">{config.temperature.toFixed(2)}</span>
        </span>
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          disabled={!config.polish_enabled}
          bind:value={config.temperature}
        />
      </span>
    </label>

    <label class="row">
      <span class="row__label">Tone</span>
      <span class="row__control">
        <textarea
          rows="3"
          placeholder="Technical, precise, no filler"
          disabled={!config.polish_enabled}
          bind:value={config.tone_hint}
        ></textarea>
        <span class="row__hint">How finished text should sound. Optional.</span>
      </span>
    </label>

    <div class="row row--switch row--flush">
      <span class="row__copy">
        <strong>Transcribe the last capture again</strong>
        <span>Runs your current settings against the audio from your most recent dictation.</span>
      </span>
      <button type="button" class="btn" disabled={testBusy} onclick={testTranscription}>
        {testBusy ? "Transcribing…" : "Run it"}
      </button>
    </div>

    {#if testResult !== null}
      <p aria-live="polite" class="note note--ok test-result">
        {testResult || "No speech in the last capture."}
      </p>
    {/if}
    {#if testError}
      <p role="alert" class="note note--bad test-result">{testError}</p>
    {/if}
  </div>
</section>

<style>
  .test-result {
    margin-block-start: var(--space-sm);
  }
</style>
