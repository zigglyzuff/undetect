<script>
  import { Button, Modal } from 'flowbite-svelte';
  import { QrCodeOutline } from 'flowbite-svelte-icons';

  export let isOpen = false;
  export let qrDataUrl = '';
  export let targetUrl = '';
  export let onClose = () => {};
</script>

<Modal
  bind:open={isOpen}
  oncancel={onClose}
  ontoggle={(e) => {
    if (e.newState !== 'open') onClose();
  }}
  size="xs"
  class="max-w-[310px] bg-zinc-900 dark:bg-zinc-900 border border-zinc-800 dark:border-zinc-800 rounded-2xl shadow-2xl text-zinc-100 dark:text-zinc-100"
  classes={{
    body: 'p-4 flex flex-col items-center text-center gap-2.5',
    close: 'text-zinc-400 hover:text-white bg-zinc-800/80 hover:bg-zinc-700 rounded-full focus:ring-0',
  }}
>
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

  <!-- URL Badge (Monochrome) -->
  <p class="text-[10px] text-zinc-200 break-all max-w-full font-mono bg-black/60 px-2.5 py-1.5 rounded-lg border border-zinc-800/80 select-all">
    {targetUrl}
  </p>

  <!-- Dismiss Button -->
  <Button
    color="alternative"
    size="xs"
    onclick={onClose}
    class="w-full mt-0.5 py-2 px-4 rounded-lg bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none text-xs font-semibold cursor-pointer transition-colors shadow focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
  >
    <span class="text-zinc-950">Done</span>
  </Button>
</Modal>
