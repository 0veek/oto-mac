<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    IconAlertTriangle,
    IconCircleCheck,
    IconCircleX,
    IconHelpCircle,
    IconRefresh,
    IconShieldCheck,
  } from "@tabler/icons-svelte";

  type PermissionItem = {
    id: string;
    name: string;
    required: boolean;
    status: string;
    detail: string;
    canOpenSettings: boolean;
  };

  type PermissionsReport = {
    allRequiredGranted: boolean;
    displayName: string;
    executablePath: string;
    bundled: boolean;
    checkedAtMs: number;
    items: PermissionItem[];
    summary: string;
  };

  let report = $state<PermissionsReport | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  function statusLabel(status: string): string {
    switch (status) {
      case "granted":
        return "Granted";
      case "denied":
        return "Denied";
      case "not_determined":
        return "Not requested";
      case "restricted":
        return "Restricted";
      case "recommended":
        return "Recommended";
      default:
        return "Unknown";
    }
  }

  function statusTone(status: string): string {
    switch (status) {
      case "granted":
        return "text-emerald-300 bg-emerald-400/10 ring-emerald-400/25";
      case "denied":
      case "restricted":
        return "text-rose-300 bg-rose-400/10 ring-rose-400/25";
      case "recommended":
        return "text-sky-300 bg-sky-400/10 ring-sky-400/25";
      case "not_determined":
        return "text-amber-200 bg-amber-400/10 ring-amber-400/25";
      default:
        return "text-slate-300 bg-white/5 ring-white/10";
    }
  }

  async function refresh() {
    busy = true;
    error = null;
    try {
      report = await invoke<PermissionsReport>("check_permissions");
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function openSettings(id: string) {
    busy = true;
    error = null;
    try {
      await invoke("open_permission_settings_cmd", { id });
      // Re-check after user may have toggled permissions.
      await new Promise((r) => setTimeout(r, 600));
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function requestAccessibility() {
    busy = true;
    error = null;
    try {
      report = await invoke<PermissionsReport>("request_accessibility_permission");
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function requestMicrophone() {
    busy = true;
    error = null;
    try {
      report = await invoke<PermissionsReport>("request_microphone_permission");
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function revealApp() {
    busy = true;
    error = null;
    try {
      await invoke("reveal_app_in_finder");
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void refresh();
    const id = setInterval(() => {
      void refresh();
    }, 4000);
    return () => clearInterval(id);
  });
</script>

<section class="space-y-6">
  <header>
    <h2 class="text-xl font-semibold tracking-tight">Permissions</h2>
    <p class="mt-1 text-sm text-slate-400">
      Check whether macOS has granted Oto everything it needs for dictation and text insertion.
    </p>
  </header>

  <div
    class="space-y-5 rounded-2xl border border-white/10 bg-white/[0.04] p-6 shadow-xl backdrop-blur-xl"
  >
    <div
      class="flex flex-wrap items-start justify-between gap-3 rounded-xl border px-4 py-3 {report?.allRequiredGranted
        ? 'border-emerald-400/25 bg-emerald-400/5'
        : 'border-amber-400/25 bg-amber-400/5'}"
    >
      <div class="flex items-start gap-3">
        {#if report?.allRequiredGranted}
          <IconShieldCheck class="mt-0.5 shrink-0 text-emerald-300" size={22} stroke={1.7} />
        {:else}
          <IconAlertTriangle class="mt-0.5 shrink-0 text-amber-200" size={22} stroke={1.7} />
        {/if}
        <div>
          <p
            class="text-sm font-medium {report?.allRequiredGranted
              ? 'text-emerald-100'
              : 'text-amber-100'}"
          >
            {report ? report.summary : "Checking permissions…"}
          </p>
          {#if report}
            <p class="mt-1 text-xs text-slate-400">
              App: <span class="font-mono text-slate-300">{report.displayName}</span>
              · {report.bundled ? "bundled Oto.app" : "dev binary"}
            </p>
          {/if}
        </div>
      </div>
      <button
        type="button"
        class="inline-flex items-center gap-1.5 rounded-lg bg-white/10 px-3 py-1.5 text-xs font-medium text-white ring-1 ring-white/15 transition hover:bg-white/15 disabled:opacity-50"
        disabled={busy}
        onclick={refresh}
      >
        <IconRefresh size={14} stroke={1.8} />
        {busy ? "Checking…" : "Check permissions"}
      </button>
    </div>

    {#if report}
      <ul class="space-y-3">
        {#each report.items as item (item.id)}
          <li
            class="rounded-xl border border-white/10 bg-slate-900/40 px-4 py-3"
          >
            <div class="flex flex-wrap items-center justify-between gap-2">
              <div class="flex items-center gap-2">
                {#if item.status === "granted"}
                  <IconCircleCheck class="text-emerald-400" size={18} stroke={1.8} />
                {:else if item.status === "denied" || item.status === "restricted"}
                  <IconCircleX class="text-rose-400" size={18} stroke={1.8} />
                {:else}
                  <IconHelpCircle class="text-amber-300" size={18} stroke={1.8} />
                {/if}
                <span class="text-sm font-medium text-slate-100">{item.name}</span>
                {#if item.required}
                  <span class="rounded bg-white/5 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-slate-400"
                    >Required</span
                  >
                {:else}
                  <span class="rounded bg-white/5 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-slate-500"
                    >Optional</span
                  >
                {/if}
              </div>
              <span
                class="rounded-full px-2.5 py-0.5 text-[11px] font-medium ring-1 {statusTone(
                  item.status,
                )}"
              >
                {statusLabel(item.status)}
              </span>
            </div>
            <p class="mt-2 whitespace-pre-line text-xs leading-relaxed text-slate-400">
              {item.detail}
            </p>
            <div class="mt-3 flex flex-wrap gap-2">
              {#if item.canOpenSettings}
                <button
                  type="button"
                  class="rounded-lg bg-white/10 px-3 py-1.5 text-xs font-medium text-white ring-1 ring-white/15 transition hover:bg-white/15 disabled:opacity-50"
                  disabled={busy}
                  onclick={() => openSettings(item.id)}
                >
                  Open {item.name} settings
                </button>
              {/if}
              {#if item.id === "accessibility" && item.status !== "granted"}
                <button
                  type="button"
                  class="rounded-lg bg-white/5 px-3 py-1.5 text-xs font-medium text-white/90 ring-1 ring-white/10 transition hover:bg-white/10 disabled:opacity-50"
                  disabled={busy}
                  onclick={requestAccessibility}
                >
                  Request Accessibility
                </button>
                <button
                  type="button"
                  class="rounded-lg bg-white/5 px-3 py-1.5 text-xs font-medium text-white/90 ring-1 ring-white/10 transition hover:bg-white/10 disabled:opacity-50"
                  disabled={busy}
                  onclick={revealApp}
                >
                  Reveal app in Finder
                </button>
              {/if}
              {#if item.id === "microphone" && item.status !== "granted"}
                <button
                  type="button"
                  class="rounded-lg bg-white/5 px-3 py-1.5 text-xs font-medium text-white/90 ring-1 ring-white/10 transition hover:bg-white/10 disabled:opacity-50"
                  disabled={busy}
                  onclick={requestMicrophone}
                >
                  Request microphone
                </button>
              {/if}
            </div>
          </li>
        {/each}
      </ul>

      {#if report.executablePath}
        <p class="break-all rounded-lg bg-black/20 px-3 py-2 font-mono text-[11px] text-slate-500">
          {report.executablePath}
        </p>
      {/if}
    {/if}

    {#if error}
      <p role="alert" class="text-sm text-rose-400">{error}</p>
    {/if}

    <div class="rounded-xl border border-white/10 bg-slate-900/30 px-4 py-3 text-xs leading-relaxed text-slate-400">
      <p class="font-medium text-slate-300">Tips</p>
      <ul class="mt-1.5 list-disc space-y-1 pl-4">
        <li>
          After enabling a toggle in System Settings, use <strong>Check permissions</strong> (or
          quit and reopen Oto).
        </li>
        <li>
          Rebuilds can create a new Accessibility entry — remove old <code class="rounded bg-white/5 px-1">oto</code>
          rows and add the current <strong>Oto.app</strong> with <strong>+</strong>.
        </li>
        <li>
          Insertion needs <strong>Accessibility</strong>. Dictation needs
          <strong>Microphone</strong>.
        </li>
      </ul>
    </div>
  </div>
</section>
