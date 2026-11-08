<script>
  import { invoke } from '@tauri-apps/api/core';
  import { RefreshOutline } from 'flowbite-svelte-icons';
  import logo from '../assets/logo.png';

  export let version = 'v2.0.0';
  export let onCheckUpdates = () => {};

  function handleMouseDown(e) {
    if (e.button === 0 && !e.target.closest('button') && !e.target.closest('a') && !e.target.closest('input')) {
      invoke('drag_window').catch((err) => {
        console.error('Drag window error:', err);
      });
    }
  }

  function handleClose(e) {
    e.stopPropagation();
    invoke('close_window').catch(console.error);
  }

  function handleMinimize(e) {
    e.stopPropagation();
    invoke('minimize_window').catch(console.error);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  data-tauri-drag-region
  onmousedown={handleMouseDown}
  class="h-11 bg-zinc-950 border-b border-white/10 flex items-center justify-between px-3.5 select-none shrink-0 cursor-grab active:cursor-grabbing hover:bg-zinc-900 transition-colors"
  title="Click and drag to move window"
>
  <!-- Traffic lights -->
  <div class="flex items-center gap-2" onmousedown={(e) => e.stopPropagation()}>
    <button
      type="button"
      onclick={handleClose}
      class="w-3.5 h-3.5 rounded-full bg-[#ff5f56] hover:brightness-110 active:brightness-90 transition-all border border-black/20 cursor-pointer shadow-sm"
      title="Close"
      aria-label="Close"
    ></button>
    <button
      type="button"
      onclick={handleMinimize}
      class="w-3.5 h-3.5 rounded-full bg-[#ffbd2e] hover:brightness-110 active:brightness-90 transition-all border border-black/20 cursor-pointer shadow-sm"
      title="Minimize"
      aria-label="Minimize"
    ></button>
    <div
      class="w-3.5 h-3.5 rounded-full bg-[#27c93f] opacity-40 cursor-default border border-black/20"
      title="Maximize"
    ></div>
  </div>

  <!-- Brand logo & title (drag-friendly) -->
  <div data-tauri-drag-region class="flex items-center gap-2 text-xs font-bold tracking-wide text-zinc-100 pointer-events-none select-none">
    <img src={logo} alt="Undetect Logo" class="w-5 h-5 object-contain" />
    <span class="tracking-wider uppercase font-mono">Undetect</span>
    <span class="text-[10px] font-mono font-medium px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-300 border border-zinc-700/60">
      {version}
    </span>
  </div>

  <!-- Header Action -->
  <div onmousedown={(e) => e.stopPropagation()}>
    <button
      type="button"
      onclick={onCheckUpdates}
      class="text-[11px] font-medium border border-zinc-700 bg-zinc-900 hover:bg-zinc-800 text-zinc-300 hover:text-white px-2.5 py-1 rounded-md flex items-center gap-1.5 transition-colors cursor-pointer shadow-sm active:scale-95"
      title="Check for updates"
    >
      <RefreshOutline class="w-3 h-3 text-zinc-400" />
      <span>Updates</span>
    </button>
  </div>
</div>
