<script>
  import { invoke } from '@tauri-apps/api/core';
  import { Badge, Button, Card, Tooltip } from 'flowbite-svelte';
  import {
    PlayOutline,
    StopOutline,
    InfoCircleOutline,
    QrCodeOutline
  } from 'flowbite-svelte-icons';
  import { TOOLTIP_CLASS } from '../lib/tooltip.js';

  export let running = false;
  export let port = 9090;
  export let password = '1234';
  export let localUrls = [];
  export let onStart = (p, pass) => {};
  export let onStop = () => {};
  export let onOpenSettings = () => {};

  let qrDataUrl = '';
  let lastQrUrl = '';
  let loadingQr = false;

  $: primaryUrl = running && localUrls && localUrls.length > 0 ? localUrls[0] : '';

  $: if (primaryUrl !== lastQrUrl) {
    lastQrUrl = primaryUrl;
    if (primaryUrl) {
      loadQr(primaryUrl);
    } else {
      qrDataUrl = '';
    }
  }

  async function loadQr(url) {
    loadingQr = true;
    try {
      qrDataUrl = await invoke('generate_qr', { text: url });
    } catch (e) {
      console.error('Failed to generate QR:', e);
      qrDataUrl = '';
    } finally {
      loadingQr = false;
    }
  }

  function handleToggleServer() {
    if (running) {
      onStop();
    } else {
      onStart(Number(port) || 9090, password);
    }
  }
</script>

<div class="flex-1 w-full h-full flex flex-col overflow-y-auto px-4 py-3 bg-zinc-950 select-none">
  <!-- Minimal Top Row: Status + Info Redirect Button -->
  <div class="w-full flex items-center justify-between shrink-0">
    {#if running}
      <Badge color="gray" class="bg-zinc-900 dark:bg-zinc-900 text-zinc-100 dark:text-zinc-100 border border-zinc-700 dark:border-zinc-700 rounded-full px-2.5 py-0.5 text-[11px] font-sans font-medium leading-none inline-flex items-center gap-1.5">
        <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.65)] animate-pulse"></span>
        <span>Online</span>
      </Badge>
    {:else}
      <Badge color="gray" class="bg-zinc-900 dark:bg-zinc-900 text-zinc-400 dark:text-zinc-400 border border-zinc-800 dark:border-zinc-800 rounded-full px-2.5 py-0.5 text-[11px] font-sans font-medium leading-none inline-flex items-center gap-1.5">
        <span class="w-1.5 h-1.5 rounded-full bg-zinc-600"></span>
        <span>Offline</span>
      </Badge>
    {/if}

    <Button
      id="remote-settings-info-btn"
      color="alternative"
      size="xs"
      onclick={onOpenSettings}
      class="p-1.5 rounded-full bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 dark:hover:bg-zinc-800 border-zinc-800 dark:border-zinc-800 text-zinc-400 dark:text-zinc-400 hover:text-white dark:hover:text-white focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0 cursor-pointer"
      aria-label="Configure server and window settings"
    >
      <InfoCircleOutline class="w-4 h-4" />
    </Button>
    <Tooltip triggeredBy="#remote-settings-info-btn" placement="left" class={TOOLTIP_CLASS}>
      Server &amp; window settings
    </Tooltip>
  </div>

  <!-- Fixed-Position Circle Power Button (never shifts up or down on toggle) -->
  <div class="w-full pt-8 pb-6 flex justify-center shrink-0">
    <Button
      pill
      color="alternative"
      onclick={handleToggleServer}
      class="w-34 h-34 rounded-full flex flex-col items-center justify-center gap-2 transition-transform active:scale-95 cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0 {running ? 'bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 dark:hover:bg-zinc-800 text-white dark:text-white hover:text-white dark:hover:text-white focus-within:text-white dark:focus-within:text-white border-2 border-white dark:border-white shadow-[0_0_28px_rgba(255,255,255,0.14)]' : 'bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none shadow-[0_0_32px_rgba(255,255,255,0.12)]'}"
    >
      {#if running}
        <StopOutline class="w-7 h-7 text-white pointer-events-none" />
        <span class="text-xs font-bold tracking-wide text-white pointer-events-none">Stop Server</span>
      {:else}
        <PlayOutline class="w-7 h-7 text-zinc-950 pointer-events-none" />
        <span class="text-xs font-bold tracking-wide text-zinc-950 pointer-events-none">Start Server</span>
      {/if}
    </Button>
  </div>

  <!-- Fixed Top-Anchored QR Card Slot Below Button -->
  <div class="w-full flex flex-col items-center justify-start">
    <Card
      size="xl"
      color="gray"
      class="w-full max-w-[290px] bg-zinc-900/60 dark:bg-zinc-900/60 border border-zinc-800 dark:border-zinc-800 rounded-2xl p-4 flex flex-col items-center text-center gap-3 shadow-xl"
    >
      {#if running}
        <div class="p-2.5 bg-white rounded-xl shadow-lg">
          {#if qrDataUrl && !loadingQr}
            <img src={qrDataUrl} alt="Remote Viewer QR Code" class="w-40 h-40 object-contain" />
          {:else}
            <div class="w-40 h-40 flex items-center justify-center text-zinc-500 text-xs font-mono">
              Generating QR...
            </div>
          {/if}
        </div>
        <p class="text-xs font-medium text-zinc-300">Scan the QR for remote access</p>
      {:else}
        <div class="w-12 h-12 rounded-xl bg-zinc-950 border border-zinc-800 flex items-center justify-center text-zinc-500">
          <QrCodeOutline class="w-6 h-6" />
        </div>
        <div>
          <p class="text-xs font-semibold text-zinc-200">Remote Access Ready</p>
          <p class="text-[11px] text-zinc-400 mt-1">
            Tap <b class="text-zinc-200">Start Server</b> above to show the QR code.
          </p>
        </div>
      {/if}
    </Card>
  </div>
</div>
