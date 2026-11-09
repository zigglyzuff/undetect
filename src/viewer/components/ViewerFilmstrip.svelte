<script>
  import { CameraPhotoOutline, TrashBinOutline } from 'flowbite-svelte-icons';

  export let screenshots = [];
  export let currentSelectedId = null;
  export let currentPass = '';
  export let onSelect = (id, timestamp) => {};
  export let onCapture = () => {};
  export let onClear = () => {};
  export let onDownload = () => {};
  export let onBatchDownload = () => {};

  let trackEl;

  function getThumbUrl(id) {
    return `/api/screenshot?id=${id}&thumb=1${currentPass ? '&pass=' + encodeURIComponent(currentPass) : ''}`;
  }

  // Auto-scroll the newly selected item into the center of the reel
  $: if (currentSelectedId && trackEl) {
    const activeEl = trackEl.querySelector(`[data-id="${currentSelectedId}"]`);
    if (activeEl) {
      activeEl.scrollIntoView({ behavior: 'smooth', inline: 'center', block: 'nearest' });
    }
  }
</script>

<div class="w-full bg-zinc-950 border-t border-white/10 flex flex-col select-none shrink-0 backdrop-blur-xl">
  <!-- Horizontal Reel Row with Edge Fades (No Arrows Needed) -->
  <div class="w-full px-2 pt-2 pb-1 relative overflow-hidden flex items-center">
    <!-- Left Edge Fade Effect -->
    <div class="absolute left-0 top-0 bottom-0 w-8 bg-gradient-to-r from-zinc-950 via-zinc-950/80 to-transparent pointer-events-none z-10"></div>
    
    <!-- Right Edge Fade Effect -->
    <div class="absolute right-0 top-0 bottom-0 w-8 bg-gradient-to-l from-zinc-950 via-zinc-950/80 to-transparent pointer-events-none z-10"></div>

    <!-- Scrollable Reel Track -->
    <div
      bind:this={trackEl}
      class="flex-1 flex items-center gap-2 overflow-x-auto overflow-y-hidden py-1 px-4 scroll-smooth"
      style="scrollbar-width: none;"
    >
      {#if screenshots.length === 0}
        <div class="text-xs text-zinc-500 py-3 w-full text-center font-mono">
          No captures in reel
        </div>
      {:else}
        {#each screenshots as s, idx}
          {@const isActive = s.id === currentSelectedId}
          <button
            type="button"
            data-id={s.id}
            onclick={() => onSelect(s.id, s.timestamp)}
            class={isActive
              ? "group relative flex items-center gap-2 px-2 py-1.5 rounded-lg border-2 border-white bg-zinc-800 text-white shadow-xl shrink-0 cursor-pointer transition-all scale-102"
              : "group relative flex items-center gap-2 px-2 py-1.5 rounded-lg border border-zinc-800 bg-zinc-900/60 hover:bg-zinc-850 hover:border-zinc-700 text-zinc-400 hover:text-zinc-200 opacity-60 hover:opacity-100 shrink-0 cursor-pointer transition-all"}
            title={`Shot #${idx + 1} at ${s.timestamp}`}
          >
            <!-- Thumbnail preview -->
            <div class="w-14 h-9 rounded overflow-hidden bg-black border border-white/10 shrink-0 relative">
              <img
                src={getThumbUrl(s.id)}
                alt="Capture thumbnail"
                loading="lazy"
                class="w-full h-full object-cover"
              />
              {#if isActive}
                <div class="absolute inset-0 ring-1 ring-inset ring-white/60"></div>
              {/if}
            </div>

            <!-- Metadata -->
            <div class="flex flex-col text-left">
              <div class="flex items-center gap-1">
                <span class="text-[9px] font-mono px-1 py-0.2 rounded bg-black/60 text-zinc-300 font-semibold">
                  #{idx + 1}
                </span>
                {#if isActive}
                  <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                {/if}
              </div>
              <span class="text-[10px] font-mono mt-0.5 whitespace-nowrap {isActive ? 'font-semibold text-white' : 'text-zinc-400'}">
                {s.timestamp}
              </span>
            </div>
          </button>
        {/each}
      {/if}
    </div>
  </div>

  <!-- Bottom Ergonomic Control Dock (Right at Thumb Reach) -->
  <div class="w-full py-2.5 px-4 sm:px-8 flex items-center justify-between max-w-lg mx-auto">
    <!-- Left Controls: Clear Session & Download All -->
    <div class="flex items-center gap-2.5">
      <!-- Clear Button (Smaller beside the shutter) -->
      <button
        type="button"
        onclick={onClear}
        disabled={screenshots.length === 0}
        class="w-10 h-10 rounded-full bg-zinc-900 border border-zinc-800 hover:border-rose-900/60 text-zinc-400 hover:text-rose-400 hover:bg-rose-950/30 disabled:opacity-20 disabled:pointer-events-none flex items-center justify-center transition-all cursor-pointer shadow-sm active:scale-95"
        title="Clear session history"
        aria-label="Clear session history"
      >
        <TrashBinOutline class="w-4 h-4" />
      </button>

      <!-- Download All (Batch Download) Button -->
      <button
        type="button"
        onclick={onBatchDownload}
        disabled={screenshots.length === 0}
        class="h-10 px-3 rounded-full bg-zinc-900 border border-zinc-800 hover:border-zinc-700 text-zinc-300 hover:text-white disabled:opacity-20 disabled:pointer-events-none flex items-center gap-1.5 transition-all cursor-pointer shadow-sm active:scale-95 text-xs font-medium"
        title="Download all screenshots in session"
      >
        <svg class="w-4 h-4 text-zinc-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"></path></svg>
        <span class="hidden sm:inline">All</span>
        <span class="text-[10px] font-mono text-zinc-400 bg-zinc-800 px-1.5 py-0.2 rounded font-semibold">
          {screenshots.length}
        </span>
      </button>
    </div>

    <!-- Center: Prominent Camera-Style Shutter Capture Button -->
    <button
      type="button"
      onclick={onCapture}
      class="w-13 h-13 rounded-full bg-white text-zinc-950 ring-4 ring-white/20 hover:ring-white/40 hover:scale-105 active:scale-90 transition-all flex items-center justify-center shadow-2xl cursor-pointer shrink-0 mx-2"
      title="Capture Screen Now"
      aria-label="Capture screen"
    >
      <div class="w-11 h-11 rounded-full border-2 border-zinc-950/20 flex items-center justify-center">
        <CameraPhotoOutline class="w-6 h-6 text-zinc-950" />
      </div>
    </button>

    <!-- Right Control: Download Current (Closer to the right thumb) -->
    <div class="flex items-center gap-2 justify-end">
      <button
        type="button"
        onclick={onDownload}
        disabled={!currentSelectedId}
        class="h-10 px-3.5 sm:px-4 rounded-full bg-white hover:bg-zinc-200 active:bg-zinc-300 text-zinc-950 font-semibold text-xs flex items-center gap-1.5 transition-all cursor-pointer shadow-lg active:scale-95 disabled:opacity-20 disabled:pointer-events-none"
        title="Download current screenshot"
      >
        <svg class="w-4 h-4 text-zinc-950" fill="none" stroke="currentColor" stroke-width="2.2" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"></path></svg>
        <span>Download</span>
      </button>
    </div>
  </div>
</div>
