<script>
  import { invoke } from '@tauri-apps/api/core';
  import { Badge, Button } from 'flowbite-svelte';
  import { CloseOutline, CogOutline } from 'flowbite-svelte-icons';
  import logo from '../assets/logo.png';

  export let version = 'v2.1.0';
  export let settingsOpen = false;
  export let clickThrough = false;
  export let onToggleSettings = () => {};
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  data-tauri-drag-region
  onmousedown={(e) => {
    if (e.button === 0 && !e.target.closest('button') && !e.target.closest('a') && !e.target.closest('input')) {
      invoke('drag_window').catch(console.error);
    }
  }}
  class="relative h-10 bg-zinc-950 border-b border-zinc-800 flex items-center justify-end px-3 select-none shrink-0 cursor-grab active:cursor-grabbing hover:bg-zinc-900/50 transition-colors"
>
  <!-- Brand logo & title -->
  <div data-tauri-drag-region class="absolute left-1/2 -translate-x-1/2 flex items-center gap-2 text-xs font-bold tracking-wide text-zinc-100 pointer-events-none select-none">
    <img src={logo} alt="Undetect Logo" class="w-4 h-4 object-contain" />
    <span class="tracking-wider uppercase font-sans font-bold text-[11px]">Undetect</span>
    <Badge color="gray" class="text-[10px] font-sans font-medium tabular-nums px-2 py-0.5 leading-none rounded-full bg-zinc-800 dark:bg-zinc-800 text-zinc-300 dark:text-zinc-300 border border-zinc-700/70 dark:border-zinc-700/70">
      {version}
    </Badge>
    {#if clickThrough}
      <Badge color="gray" class="text-[10px] font-sans font-medium px-2 py-0.5 leading-none rounded-full bg-emerald-500/15 dark:bg-emerald-500/15 text-emerald-300 dark:text-emerald-300 border border-emerald-500/40 dark:border-emerald-500/40">
        Ghost
      </Badge>
    {/if}
  </div>

  <!-- Top-Right Controls: Settings + Exit App -->
  <div class="flex items-center gap-1.5" onmousedown={(e) => e.stopPropagation()}>
    <Button
      color="alternative"
      size="xs"
      onclick={onToggleSettings}
      class="p-1.5 rounded-lg transition-colors cursor-pointer shadow-sm focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0 {settingsOpen ? 'bg-white dark:bg-white text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-white dark:border-white hover:bg-zinc-200 dark:hover:bg-zinc-200' : 'bg-zinc-900 dark:bg-zinc-900 border-zinc-800 dark:border-zinc-800 text-zinc-400 dark:text-zinc-400 hover:text-white dark:hover:text-white hover:bg-zinc-800 dark:hover:bg-zinc-800'}"
      title={settingsOpen ? 'Close Settings' : 'Settings'}
      aria-label="Settings"
    >
      <CogOutline class="w-4 h-4 {settingsOpen ? 'text-zinc-950' : ''}" />
    </Button>

    <Button
      color="alternative"
      size="xs"
      onclick={() => invoke('close_window').catch(console.error)}
      class="p-1.5 rounded-lg bg-zinc-900 dark:bg-zinc-900 border-zinc-800 dark:border-zinc-800 text-zinc-400 dark:text-zinc-400 hover:text-white dark:hover:text-white hover:bg-zinc-800 dark:hover:bg-zinc-800 transition-colors cursor-pointer shadow-sm focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
      title="Exit App"
      aria-label="Exit App"
    >
      <CloseOutline class="w-4 h-4" />
    </Button>
  </div>
</div>
