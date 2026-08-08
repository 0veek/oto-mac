<script lang="ts">
  import type { AppConfig } from "$lib/types";

  let { config }: { config: AppConfig } = $props();

  const INJECTION_LABELS: Record<string, string> = {
    auto: "Automatic",
    direct_type: "Typed",
    clipboard_paste: "Clipboard + paste",
    clipboard_only: "Clipboard only",
  };

  const activeProfile = $derived(
    config.provider_preset === "custom" && config.active_custom_provider_id
      ? (config.custom_providers.find((p) => p.id === config.active_custom_provider_id) ?? null)
      : null,
  );

  // The chain reports what is actually configured, so it stays a readout rather
  // than a diagram of the idea of a pipeline.
  const stages = $derived([
    {
      name: "Capture",
      value: config.audio.input_device ?? "System default",
      active: true,
    },
    {
      name: "Transcribe",
      value:
        config.stt_backend === "local_whisper"
          ? "Local Whisper"
          : (activeProfile?.stt_model || config.stt_model || "Not set"),
      active: true,
    },
    {
      name: "Clean up",
      value: config.polish_enabled
        ? (activeProfile?.polish_model || config.polish_model || "Not set")
        : "Off",
      active: config.polish_enabled,
    },
    {
      name: "Insert",
      value: INJECTION_LABELS[config.injection_mode] ?? config.injection_mode,
      active: true,
    },
  ]);
</script>

<div class="chain" aria-label="Configured dictation path">
  {#each stages as stage, index (stage.name)}
    <div class="chain__stage" data-active={stage.active}>
      <span class="plate-micro chain__index">{String(index + 1).padStart(2, "0")}</span>
      <span class="chain__name">{stage.name}</span>
      <span class="chain__value" title={stage.value}>{stage.value}</span>
    </div>
  {/each}
</div>
