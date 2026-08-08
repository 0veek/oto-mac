<script lang="ts">
  import { onMount } from "svelte";
  import type { Snippet } from "svelte";
  import {
    IconBook2,
    IconBox,
    IconChartBar,
    IconChevronDown,
    IconContrast2,
    IconCursorText,
    IconCut,
    IconHandStop,
    IconHistory,
    IconInfoCircle,
    IconKeyboard,
    IconLayoutGrid,
    IconMicrophone,
    IconSearch,
    IconShieldCheck,
    IconTypography,
    IconWaveSine,
  } from "@tabler/icons-svelte";
  import Gauge from "../Gauge.svelte";

  let {
    sections,
    active,
    theme,
    onselect,
    children,
  }: {
    sections: { id: string; label: string }[];
    active: string;
    theme: string;
    onselect: (id: string) => void;
    children: Snippet;
  } = $props();

  let query = $state("");
  let searchInput: HTMLInputElement;

  // Resolves to ⌘F here, which is what this port wants — but the check stays at
  // runtime because the same component is shared with the Linux/Windows builds.
  const searchShortcutLabel =
    typeof navigator !== "undefined" && /mac/i.test(navigator.platform ?? navigator.userAgent)
      ? "⌘F"
      : "^F";

  const sectionIcons = {
    providers: IconWaveSine,
    models: IconBox,
    hotkeys: IconKeyboard,
    audio: IconMicrophone,
    modes: IconLayoutGrid,
    injection: IconCursorText,
    dictionary: IconBook2,
    snippets: IconCut,
    styles: IconTypography,
    history: IconHistory,
    stats: IconChartBar,
    permissions: IconShieldCheck,
    appearance: IconContrast2,
    privacy: IconHandStop,
    about: IconInfoCircle,
  };

  // Group names follow the signal path rather than the file layout: what Oto
  // hears, what it writes, and the machine it runs on.
  const groups = [
    { label: "Capture", ids: ["providers", "models", "hotkeys", "audio", "modes", "injection"] },
    { label: "Text", ids: ["dictionary", "snippets", "styles", "history", "stats"] },
    { label: "System", ids: ["permissions", "appearance", "privacy", "about"] },
  ];

  function iconFor(id: string) {
    return sectionIcons[id as keyof typeof sectionIcons] ?? IconInfoCircle;
  }

  function navLabelFor(section: { id: string; label: string }) {
    if (section.id === "styles") return "Styles";
    if (section.id === "privacy") return "Privacy";
    return section.label;
  }

  function visibleSections(ids: string[]) {
    const normalizedQuery = query.trim().toLocaleLowerCase();
    return sections.filter((section) => {
      if (!ids.includes(section.id)) return false;
      return !normalizedQuery || navLabelFor(section).toLocaleLowerCase().includes(normalizedQuery);
    });
  }

  const noMatches = $derived(
    query.trim().length > 0 && groups.every((group) => visibleSections(group.ids).length === 0),
  );

  onMount(() => {
    const focusSearch = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLocaleLowerCase() === "f") {
        event.preventDefault();
        searchInput?.focus();
        searchInput?.select();
      }
    };
    window.addEventListener("keydown", focusSearch);
    return () => window.removeEventListener("keydown", focusSearch);
  });
</script>

<!-- The window runs with `titleBarStyle: "Overlay"` and a hidden title, so the
     only drag handle is whatever the page marks itself. Both shells that can own
     the top-left corner carry `data-tauri-drag-region`, and each non-interactive
     child repeats it — Tauri tests the attribute on the event target itself, not
     on its ancestors, so an unmarked child would swallow the drag. -->
<div class="oto-app" data-theme={theme}>
  <header class="topbar" data-tauri-drag-region>
    <img
      class="rail__mark"
      src="/favicon.png"
      alt=""
      width="22"
      height="22"
      data-tauri-drag-region
    />
    <div class="topbar__copy" data-tauri-drag-region>
      <span class="topbar__brand" data-tauri-drag-region>Oto</span>
      <span class="topbar__section" data-tauri-drag-region>
        {sections.find((section) => section.id === active)?.label ?? "Settings"}
      </span>
    </div>
    <div class="select-wrap">
      <select
        aria-label="Settings section"
        value={active}
        onchange={(event) => onselect(event.currentTarget.value)}
      >
        {#each sections as section (section.id)}
          <option value={section.id}>{section.label}</option>
        {/each}
      </select>
      <IconChevronDown aria-hidden="true" size={14} stroke={1.7} />
    </div>
  </header>

  <aside class="rail">
    <div class="rail__plate" data-tauri-drag-region>
      <img
        class="rail__mark"
        src="/favicon.png"
        alt=""
        width="22"
        height="22"
        data-tauri-drag-region
      />
      <span class="rail__wordmark" data-tauri-drag-region>Oto</span>
    </div>

    <Gauge />

    <label class="rail-search">
      <IconSearch aria-hidden="true" size={15} stroke={1.7} />
      <input
        bind:this={searchInput}
        bind:value={query}
        type="search"
        placeholder="Search settings"
        aria-label="Search settings"
      />
      <kbd>{searchShortcutLabel}</kbd>
    </label>

    <nav class="rail__nav" aria-label="Settings sections">
      {#each groups as group (group.label)}
        {@const matches = visibleSections(group.ids)}
        {#if matches.length}
          <div class="nav-group">
            <p class="plate-micro nav-group__label">{group.label}</p>
            {#each matches as section (section.id)}
              {@const SectionIcon = iconFor(section.id)}
              <button
                class="nav-item"
                type="button"
                data-active={active === section.id}
                aria-current={active === section.id ? "page" : undefined}
                onclick={() => onselect(section.id)}
              >
                <SectionIcon aria-hidden="true" size={17} stroke={1.6} />
                <span class="nav-item__label">{navLabelFor(section)}</span>
              </button>
            {/each}
          </div>
        {/if}
      {/each}

      {#if noMatches}
        <p class="rail-empty">Nothing matches “{query.trim()}”.</p>
      {/if}
    </nav>
  </aside>

  <main class="stage">
    {@render children()}
  </main>
</div>
