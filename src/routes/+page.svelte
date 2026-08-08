<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import FloatingPill from "$lib/components/FloatingPill.svelte";
  import { applyPipelineEvent } from "$lib/stores/pipeline";
  import type { AppConfig, PipelineEvent } from "$lib/types";
  import { invoke } from "@tauri-apps/api/core";
  import { applyTheme } from "$lib/theme";

  // Window show/hide is owned by the Rust pipeline. The frontend must NOT
  // call hide() on mount — a cold-start overlay loads with state "idle" and
  // would immediately hide itself, racing the Listening event.
  //
  // Drag position is persisted by the Rust `WindowEvent::Moved` handler in
  // lib.rs, which macOS reaches through `performWindowDragWithEvent`. Doing it
  // here as well ran two independent load-modify-save cycles over config.json
  // at once, which could drop unrelated fields.

  onMount(() => {
    document.documentElement.dataset.surface = "overlay";
    document.body.dataset.surface = "overlay";

    void invoke<AppConfig>("get_config")
      .then((config) => applyTheme(config.theme, config.reduce_motion, config.font_scale))
      .catch(() => {});
    // `.catch` is attached at subscribe time: outside a Tauri webview `listen`
    // rejects immediately and would surface as an unhandled rejection.
    const unlistenPromise = listen<PipelineEvent>("pipeline://event", (e) => {
      applyPipelineEvent(e.payload);
    }).catch(() => null);

    return () => {
      delete document.documentElement.dataset.surface;
      delete document.body.dataset.surface;
      void unlistenPromise.then((stop) => stop?.());
    };
  });
</script>

<div class="overlay-host">
  <FloatingPill />
</div>
