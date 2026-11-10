<script>
  import { Badge, Button, Card } from 'flowbite-svelte';
  import { QrCodeOutline, FileCopyOutline, InfoCircleOutline } from 'flowbite-svelte-icons';

  export let localUrls = [];
  export let onOpenQr = (url) => {};
  export let onCopyUrl = (url) => {};
</script>

<Card
  size="xl"
  color="gray"
  class="w-full bg-zinc-900/60 dark:bg-zinc-900/60 border border-zinc-800 dark:border-zinc-800 rounded-xl p-4 flex flex-col gap-3 shadow-xl"
>
  <div class="text-sm font-semibold text-zinc-100 flex items-center justify-between">
    <span>Local Wi-Fi Access URLs</span>
    {#if localUrls && localUrls.length > 0}
      <Badge color="gray" class="bg-zinc-800 dark:bg-zinc-800 text-zinc-300 dark:text-zinc-300 border border-zinc-700 dark:border-zinc-700 text-[10px] font-mono font-medium">
        {localUrls.length} online
      </Badge>
    {/if}
  </div>

  <div class="flex flex-col gap-2 max-h-36 overflow-y-auto">
    {#if localUrls && localUrls.length > 0}
      {#each localUrls as url}
        <div class="flex items-center justify-between gap-2 bg-black/50 border border-zinc-800/80 rounded-lg p-2 font-mono text-[11px]">
          <span class="text-zinc-200 font-mono font-medium break-all select-all flex-1">{url}</span>
          <div class="flex items-center gap-1.5 shrink-0">
            <Button
              color="alternative"
              size="xs"
              onclick={() => onOpenQr(url)}
              class="px-2 py-1 rounded bg-zinc-800 dark:bg-zinc-800 hover:bg-zinc-700 dark:hover:bg-zinc-700 border-zinc-700 dark:border-zinc-700 text-zinc-300 dark:text-zinc-300 text-[10px] font-medium flex items-center gap-1 cursor-pointer focus:ring-0"
              title="View QR Code"
            >
              <QrCodeOutline class="w-3 h-3 text-zinc-400" />
              <span>QR</span>
            </Button>
            <Button
              color="alternative"
              size="xs"
              onclick={() => onCopyUrl(url)}
              class="px-2 py-1 rounded bg-zinc-800 dark:bg-zinc-800 hover:bg-zinc-700 dark:hover:bg-zinc-700 border-zinc-700 dark:border-zinc-700 text-zinc-300 dark:text-zinc-300 text-[10px] font-medium flex items-center gap-1 cursor-pointer focus:ring-0"
              title="Copy to Clipboard"
            >
              <FileCopyOutline class="w-3 h-3 text-zinc-400" />
              <span>Copy</span>
            </Button>
          </div>
        </div>
      {/each}
    {:else}
      <div class="text-xs text-zinc-500 py-1 text-center font-mono">Server stopped or no active network adapter</div>
    {/if}
  </div>

  <div class="text-[11px] text-zinc-400 mt-0.5 flex items-center gap-1.5 bg-black/30 p-2 rounded-lg border border-white/5">
    <InfoCircleOutline class="w-3.5 h-3.5 text-zinc-400 shrink-0" />
    <span>Shortcut: <b class="text-zinc-200 font-semibold">Ctrl+Alt+S</b> captures primary display silently.</span>
  </div>
</Card>
