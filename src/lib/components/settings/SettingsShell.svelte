<script lang="ts">
  import type { Snippet } from "svelte";
  import {
    IconBook2,
    IconBox,
    IconBraces,
    IconChartBar,
    IconChevronDown,
    IconCursorText,
    IconHistory,
    IconInfoCircle,
    IconKeyboard,
    IconLayoutGrid,
    IconMicrophone,
    IconPalette,
    IconSearch,
    IconServer,
    IconSettings,
    IconShieldCheck,
    IconShieldLock,
    IconWand,
  } from "@tabler/icons-svelte";

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

  const sectionIcons = {
    providers: IconServer,
    models: IconBox,
    hotkeys: IconKeyboard,
    audio: IconMicrophone,
    modes: IconLayoutGrid,
    dictionary: IconBook2,
    snippets: IconBraces,
    styles: IconWand,
    history: IconHistory,
    stats: IconChartBar,
    appearance: IconPalette,
    privacy: IconShieldLock,
    permissions: IconShieldCheck,
    injection: IconCursorText,
    about: IconInfoCircle,
  };

  const groups = [
    { label: "Voice", ids: ["providers", "models", "hotkeys", "audio", "modes", "injection"] },
    { label: "Writing", ids: ["dictionary", "snippets", "styles", "history", "stats"] },
    { label: "System", ids: ["permissions", "appearance", "privacy", "about"] },
  ];

  let navQuery = $state("");

  function iconFor(id: string) {
    return sectionIcons[id as keyof typeof sectionIcons] ?? IconSettings;
  }

  function navLabelFor(section: { id: string; label: string }) {
    return section.id === "styles" ? "Styles" : section.label;
  }

  function visibleSections(ids: string[]) {
    const query = navQuery.trim().toLocaleLowerCase();
    return sections.filter(
      (section) =>
        ids.includes(section.id) &&
        (!query || navLabelFor(section).toLocaleLowerCase().includes(query)),
    );
  }
</script>

<div class="oto-settings" data-theme={theme}>
  <header class="settings-titlebar" data-tauri-drag-region>
    <span data-tauri-drag-region>Oto</span>
  </header>

  <header class="settings-compact-nav">
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
      <IconChevronDown aria-hidden="true" size={16} stroke={1.7} />
    </div>
  </header>

  <aside class="settings-sidebar">
    <label class="settings-search">
      <IconSearch aria-hidden="true" size={15} stroke={1.8} />
      <span class="sr-only">Search settings</span>
      <input type="search" placeholder="Search" bind:value={navQuery} />
      <kbd>⌘F</kbd>
    </label>

    <nav aria-label="Settings sections">
      {#each groups as group (group.label)}
        {@const groupSections = visibleSections(group.ids)}
        {#if groupSections.length > 0}
        <div class="settings-nav-group">
          <p class="settings-nav-group__label">{group.label}</p>
          {#each groupSections as section (section.id)}
            {@const SectionIcon = iconFor(section.id)}
            <button
              class="settings-nav-button"
              type="button"
              data-active={active === section.id}
              aria-current={active === section.id ? "page" : undefined}
              onclick={() => onselect(section.id)}
            >
              <SectionIcon aria-hidden="true" size={18} stroke={1.6} />
              <span>{navLabelFor(section)}</span>
            </button>
          {/each}
        </div>
        {/if}
      {/each}
      {#if groups.every((group) => visibleSections(group.ids).length === 0)}
        <p class="settings-search-empty">No settings found</p>
      {/if}
    </nav>
  </aside>

  <main class="settings-main">
    {@render children()}
  </main>
</div>
