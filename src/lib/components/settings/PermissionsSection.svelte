<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    IconAlertTriangle,
    IconArrowRight,
    IconCircleCheck,
    IconCircleX,
    IconHelpCircle,
    IconPointer,
    IconRefresh,
    IconShieldCheck,
  } from "@tabler/icons-svelte";

  let {
    onselect,
  }: {
    /** Navigate to another settings section without a full page remount. */
    onselect?: (id: string) => void;
  } = $props();

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

  // TCC states borrow the meter's zones: safe when granted, clipped when refused,
  // lamp-lit while still waiting on you, unlit when it does not matter.
  function statusTone(status: string): "ok" | "bad" | "warn" | "idle" {
    switch (status) {
      case "granted":
        return "ok";
      case "denied":
      case "restricted":
        return "bad";
      case "not_determined":
        return "warn";
      default:
        return "idle";
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

  // A toggle flipped in System Settings never notifies us, so the report is polled
  // while this section is on screen.
  onMount(() => {
    void refresh();
    const id = setInterval(() => {
      void refresh();
    }, 4000);
    return () => clearInterval(id);
  });
</script>

<section class="section">
  <header class="section__head">
    <h2 class="section__title">Permissions</h2>
    <p class="section__lead">
      What macOS has granted Oto: the microphone it listens through, and the
      Accessibility API it types with.
    </p>
  </header>

  <div
    role="status"
    class="note"
    class:note--ok={report?.allRequiredGranted}
    class:note--warn={report != null && !report.allRequiredGranted}
  >
    {#if report?.allRequiredGranted}
      <IconShieldCheck aria-hidden="true" size={16} stroke={1.7} />
    {:else}
      <IconAlertTriangle aria-hidden="true" size={16} stroke={1.8} />
    {/if}
    <div class="note__body">
      <p><strong>{report ? report.summary : "Checking permissions…"}</strong></p>
      {#if report}
        <p>
          Listed as <span class="readout-tight">{report.displayName}</span> —
          {report.bundled ? "bundled Oto.app" : "dev binary"}.
        </p>
      {/if}
      <button type="button" class="btn btn--small" disabled={busy} onclick={refresh}>
        <IconRefresh aria-hidden="true" size={14} stroke={1.8} />
        {busy ? "Checking…" : "Check permissions"}
      </button>
    </div>
  </div>

  {#if error}
    <p role="alert" class="note note--bad">{error}</p>
  {/if}

  {#if report}
    <div class="items">
      {#each report.items as item (item.id)}
        <div class="item">
          <div class="item__head">
            <span class="perm-head">
              {#if item.status === "granted"}
                <IconCircleCheck class="status-ok" aria-hidden="true" size={16} stroke={1.8} />
              {:else if item.status === "denied" || item.status === "restricted"}
                <IconCircleX class="status-bad" aria-hidden="true" size={16} stroke={1.8} />
              {:else}
                <IconHelpCircle class="status-warn" aria-hidden="true" size={16} stroke={1.8} />
              {/if}
              <span class="item__title">{item.name}</span>
              <span class="plate-micro perm-flag">
                {item.required ? "Required" : "Optional"}
              </span>
            </span>
            <span class="plate-micro perm-state" data-tone={statusTone(item.status)}>
              {statusLabel(item.status)}
            </span>
          </div>

          <p class="item__body perm-detail">{item.detail}</p>

          {#if item.canOpenSettings || item.status !== "granted"}
            <div class="btn-row">
              {#if item.canOpenSettings}
                <button
                  type="button"
                  class="btn btn--small"
                  disabled={busy}
                  onclick={() => openSettings(item.id)}
                >
                  Open {item.name} settings
                </button>
              {/if}
              {#if item.id === "accessibility" && item.status !== "granted"}
                <button
                  type="button"
                  class="btn btn--small"
                  disabled={busy}
                  onclick={requestAccessibility}
                >
                  Request Accessibility
                </button>
                <button type="button" class="btn btn--small" disabled={busy} onclick={revealApp}>
                  Reveal app in Finder
                </button>
              {/if}
              {#if item.id === "microphone" && item.status !== "granted"}
                <button
                  type="button"
                  class="btn btn--small"
                  disabled={busy}
                  onclick={requestMicrophone}
                >
                  Request microphone
                </button>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>

    {#if report.executablePath}
      <div class="field">
        <span class="plate-micro field__label">Binary macOS is checking</span>
        <pre class="output">{report.executablePath}</pre>
      </div>
    {/if}
  {/if}

  {#if onselect}
    <div class="pairs">
      <div class="pair">
        <span class="pair__icon">
          <IconPointer aria-hidden="true" size={16} stroke={1.7} />
        </span>
        <span class="pair__copy">
          <strong>Typing into other windows</strong>
          <span>How finished text reaches the application you were using.</span>
        </span>
        <button type="button" class="btn-link pair__side" onclick={() => onselect?.("injection")}>
          Check insertion
          <IconArrowRight aria-hidden="true" size={14} stroke={1.8} />
        </button>
      </div>
    </div>
  {/if}

  <details class="disclosure">
    <summary>Oto is missing from the Accessibility list</summary>
    <div class="disclosure__body">
      <ul>
        <li>
          Run <strong>/Applications/Oto.app</strong>. Drag-copying out of the build folder usually
          breaks the signature, and an unsigned copy never appears in the list — use
          <span class="readout-tight">npm run app:install</span> from the repo instead.
        </li>
        <li>
          After flipping a toggle in System Settings, press <strong>Check permissions</strong>, or
          quit and reopen Oto from Applications.
        </li>
        <li>
          Every rebuild is a new identity to macOS. Remove the stale
          <span class="readout-tight">Oto</span> and <span class="readout-tight">oto</span> rows,
          then <strong>+</strong> add <strong>/Applications/Oto.app</strong> only.
        </li>
        <li>
          Insertion needs <strong>Accessibility</strong>; dictation needs
          <strong>Microphone</strong>.
        </li>
      </ul>
    </div>
  </details>
</section>

<style>
  .note__body {
    display: grid;
    gap: 0.5rem;
    min-width: 0;
  }

  .note__body .btn {
    justify-self: start;
  }

  .perm-head {
    display: flex;
    align-items: center;
    gap: 0.4375rem;
    min-width: 0;
  }

  .perm-flag {
    flex: 0 0 auto;
    color: var(--faint);
  }

  /* The TCC state chip: an unlit plate that takes a meter colour once macOS has
     actually decided something. */
  .perm-state {
    display: inline-flex;
    flex: 0 0 auto;
    align-items: center;
    padding: 0.1875rem 0.4375rem;
    border: var(--rule) solid var(--etch-strong);
    border-radius: var(--radius-control);
    color: var(--muted);
    background: var(--well);
  }

  .perm-state[data-tone="ok"] {
    border-color: var(--signal-safe);
    color: var(--signal-safe);
  }

  .perm-state[data-tone="bad"] {
    border-color: var(--signal-clip);
    color: var(--signal-clip);
  }

  .perm-state[data-tone="warn"] {
    border-color: var(--lamp);
    color: var(--lamp-text);
  }

  /* The backend writes these details as multiple lines. */
  .perm-detail {
    white-space: pre-line;
  }
</style>
