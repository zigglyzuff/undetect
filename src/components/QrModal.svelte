<script>
  import { QrCodeOutline, CloseOutline } from 'flowbite-svelte-icons';

  export let isOpen = false;
  export let qrDataUrl = '';
  export let targetUrl = '';
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
  <!-- Backdrop -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4 animate-in fade-in duration-200 select-none"
    onclick={handleBackdropClick}
  >
    <!-- Dialog Card (Appropriately sized for the 420px desktop window) -->
    <div
      class="w-full max-w-[310px] bg-zinc-900 border border-white/10 rounded-2xl p-4 shadow-2xl flex flex-col items-center text-center gap-2.5 relative animate-in fade-in zoom-in-95 duration-200"
    >
      <!-- Close button in top-right -->
      <button
        type="button"
        onclick={onClose}
        class="absolute top-3 right-3 w-7 h-7 rounded-full bg-zinc-800/80 hover:bg-zinc-700 text-zinc-400 hover:text-white flex items-center justify-center transition-colors cursor-pointer border border-white/5"
        title="Close dialog"
        aria-label="Close"
      >
        <CloseOutline class="w-3.5 h-3.5" />
      </button>

      <!-- Icon Header -->
      <div class="w-10 h-10 rounded-xl bg-zinc-800 border border-zinc-700 text-zinc-200 flex items-center justify-center shadow-inner mt-1">
        <QrCodeOutline class="w-5 h-5 text-zinc-200" />
      </div>

      <div>
        <h3 class="text-sm font-bold text-white tracking-wide">Scan with Phone</h3>
        <p class="text-[11px] text-zinc-400 mt-0.5 max-w-[240px]">Scan with your mobile camera to open the remote viewer instantly</p>
      </div>

      <!-- High-contrast QR Container -->
      <div class="p-2.5 bg-white rounded-xl shadow-lg my-0.5">
        {#if qrDataUrl}
          <img src={qrDataUrl} alt="Wi-Fi QR Code" class="w-38 h-38 object-contain" />
        {:else}
          <div class="w-38 h-38 flex items-center justify-center text-zinc-500 text-xs font-medium font-mono">
            Loading QR Code...
          </div>
        {/if}
      </div>

      <!-- URL Badge -->
      <p class="text-[10px] text-emerald-400 break-all max-w-full font-mono bg-black/60 px-2.5 py-1.5 rounded-lg border border-zinc-800/80 select-all">
        {targetUrl}
      </p>

      <!-- Dismiss Button -->
      <button
        type="button"
        onclick={onClose}
        class="w-full mt-0.5 py-2 px-4 rounded-lg bg-white hover:bg-zinc-200 active:bg-zinc-300 text-zinc-950 text-xs font-semibold cursor-pointer transition-colors shadow active:scale-98"
      >
        Done
      </button>
    </div>
  </div>
{/if}
