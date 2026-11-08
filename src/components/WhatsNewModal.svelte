<script>
  import { CloseOutline } from 'flowbite-svelte-icons';
  import logo from '../assets/logo.png';

  export let isOpen = false;
  export let onClose = () => {};

  function handleKeydown(e) {
    if (e.key === 'Escape' && isOpen) {
      onClose();
    }
  }

  function handleBackdropClick(e) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if isOpen}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 bg-black/85 backdrop-blur-sm flex items-center justify-center p-4 animate-in fade-in duration-200 select-none"
    onclick={handleBackdropClick}
  >
    <div
      class="w-full max-w-[340px] bg-zinc-900 border border-white/10 rounded-2xl p-5 shadow-2xl flex flex-col gap-3.5 relative animate-in fade-in zoom-in-95 duration-200"
    >
      <!-- Top Close Button -->
      <button
        type="button"
        onclick={onClose}
        class="absolute top-3 right-3 w-7 h-7 rounded-full bg-zinc-850 hover:bg-zinc-800 text-zinc-400 hover:text-white flex items-center justify-center transition-colors cursor-pointer border border-white/5"
        title="Close"
        aria-label="Close"
      >
        <CloseOutline class="w-3.5 h-3.5" />
      </button>

      <!-- Header with Logo and Badge -->
      <div class="flex items-center gap-2.5">
        <img src={logo} alt="Undetect Logo" class="w-7 h-7 object-contain" />
        <div>
          <div class="flex items-center gap-1.5">
            <h3 class="text-sm font-bold text-white tracking-wide">What's New</h3>
            <span class="text-[10px] font-mono font-semibold px-1.5 py-0.2 rounded bg-zinc-800 text-zinc-300 border border-zinc-700">
              v2.0
            </span>
          </div>
          <p class="text-[11px] text-zinc-400 mt-0.5">Ground-up rewrite for speed & stealth</p>
        </div>
      </div>

      <!-- Feature Highlights List -->
      <div class="flex flex-col gap-2.5 bg-black/40 border border-white/5 rounded-xl p-3 text-left">
        <!-- Item 1 -->
        <div class="flex items-start gap-2.5">
          <div class="w-5 h-5 rounded-md bg-zinc-800 border border-zinc-700/80 flex items-center justify-center shrink-0 mt-0.5 text-zinc-300">
            <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z"></path></svg>
          </div>
          <div class="flex flex-col">
            <span class="text-xs font-semibold text-zinc-200">Rust & Tauri v2 Core</span>
            <span class="text-[11px] text-zinc-400 leading-snug">Replaced Electron. Instant startup with a fraction of the RAM.</span>
          </div>
        </div>

        <!-- Item 2 -->
        <div class="flex items-start gap-2.5">
          <div class="w-5 h-5 rounded-md bg-zinc-800 border border-zinc-700/80 flex items-center justify-center shrink-0 mt-0.5 text-zinc-300">
            <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0zM10 7v6m3-3H7"></path></svg>
          </div>
          <div class="flex flex-col">
            <span class="text-xs font-semibold text-zinc-200">In-Place Gesture Zoom</span>
            <span class="text-[11px] text-zinc-400 leading-snug">Pinch-to-zoom, mouse wheel, 2.5× double-tap, and fluid panning.</span>
          </div>
        </div>

        <!-- Item 3 -->
        <div class="flex items-start gap-2.5">
          <div class="w-5 h-5 rounded-md bg-zinc-800 border border-zinc-700/80 flex items-center justify-center shrink-0 mt-0.5 text-zinc-300">
            <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M3 9a2 2 0 012-2h.93a2 2 0 001.664-.89l.812-1.22A2 2 0 0110.07 4h3.86a2 2 0 011.664.89l.812 1.22A2 2 0 0018.07 7H19a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V9z"></path></svg>
          </div>
          <div class="flex flex-col">
            <span class="text-xs font-semibold text-zinc-200">Ergonomic Camera Dock</span>
            <span class="text-[11px] text-zinc-400 leading-snug">Shutter button and quick download buttons right at thumb reach.</span>
          </div>
        </div>

        <!-- Item 4 -->
        <div class="flex items-start gap-2.5">
          <div class="w-5 h-5 rounded-md bg-zinc-800 border border-zinc-700/80 flex items-center justify-center shrink-0 mt-0.5 text-zinc-300">
            <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"></path></svg>
          </div>
          <div class="flex flex-col">
            <span class="text-xs font-semibold text-zinc-200">Stealth Cloaking</span>
            <span class="text-[11px] text-zinc-400 leading-snug">Excluded from screen shares and hidden from the Windows taskbar.</span>
          </div>
        </div>
      </div>

      <!-- Action Button -->
      <button
        type="button"
        onclick={onClose}
        class="w-full py-2.5 px-4 rounded-xl bg-white hover:bg-zinc-200 active:bg-zinc-300 text-zinc-950 text-xs font-semibold transition-all shadow cursor-pointer active:scale-98"
      >
        Got it, let's go
      </button>
    </div>
  </div>
{/if}
