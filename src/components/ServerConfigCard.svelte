<script>
  import { Badge, Button, Card, Input, Label } from 'flowbite-svelte';
  import { PlayOutline, StopOutline } from 'flowbite-svelte-icons';

  export let running = false;
  export let port = 9090;
  export let password = '1234';
  export let onStart = (p, pass) => {};
  export let onStop = () => {};

  let inputPort = port;
  let inputPassword = password;

  $: inputPort = port;
  $: inputPassword = password;

  function handleStart() {
    onStart(Number(inputPort) || 9090, inputPassword);
  }
</script>

<Card
  size="xl"
  color="gray"
  class="w-full bg-zinc-900/60 dark:bg-zinc-900/60 border border-zinc-800 dark:border-zinc-800 rounded-xl p-4 flex flex-col gap-3 shadow-xl"
>
  <div class="flex items-center justify-between">
    <span class="text-sm font-semibold text-zinc-100">Server Configuration</span>
    {#if running}
      <Badge color="gray" class="bg-zinc-800 dark:bg-zinc-800 text-zinc-100 dark:text-zinc-100 border border-zinc-600 dark:border-zinc-600 rounded-full px-2.5 py-0.5 text-xs font-semibold flex items-center gap-1.5">
        <span class="w-1.5 h-1.5 rounded-full bg-white shadow-[0_0_6px_#ffffff] animate-pulse"></span>
        Active
      </Badge>
    {:else}
      <Badge color="gray" class="bg-zinc-800 dark:bg-zinc-800 text-zinc-400 dark:text-zinc-400 border border-zinc-700 dark:border-zinc-700 rounded-full px-2.5 py-0.5 text-xs font-semibold flex items-center gap-1.5">
        <span class="w-1.5 h-1.5 rounded-full bg-zinc-500"></span>
        Stopped
      </Badge>
    {/if}
  </div>

  <div class="flex flex-col gap-1.5">
    <Label for="server-pin" class="text-xs font-medium text-zinc-400 dark:text-zinc-400">Access Password (PIN)</Label>
    <Input
      id="server-pin"
      type="password"
      bind:value={inputPassword}
      placeholder="Set Passcode"
      size="sm"
      class="bg-black/50 dark:bg-black/50 border-zinc-800 dark:border-zinc-800 text-zinc-100 dark:text-zinc-100 text-sm focus:border-white dark:focus:border-white focus:ring-0 dark:focus:ring-0 font-mono"
    />
  </div>

  <div class="flex flex-col gap-1.5">
    <Label for="server-port" class="text-xs font-medium text-zinc-400 dark:text-zinc-400">Port</Label>
    <Input
      id="server-port"
      type="number"
      bind:value={inputPort}
      placeholder="9090"
      size="sm"
      class="bg-black/50 dark:bg-black/50 border-zinc-800 dark:border-zinc-800 text-zinc-100 dark:text-zinc-100 text-sm focus:border-white dark:focus:border-white focus:ring-0 dark:focus:ring-0"
    />
  </div>

  <div class="grid grid-cols-2 gap-2.5 pt-1">
    <Button
      color="alternative"
      size="xs"
      onclick={handleStart}
      class="w-full px-3 py-2 bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 active:bg-zinc-300 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none rounded-lg text-xs font-semibold transition-all shadow-md flex items-center justify-center gap-1.5 cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
    >
      <PlayOutline class="w-3.5 h-3.5 text-zinc-950" />
      <span class="text-zinc-950">Update &amp; Start</span>
    </Button>
    <Button
      color="alternative"
      size="xs"
      onclick={onStop}
      class="w-full px-3 py-2 bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 dark:hover:bg-zinc-800 text-zinc-300 dark:text-zinc-300 hover:text-white border border-zinc-800 dark:border-zinc-800 rounded-lg text-xs font-semibold transition-all flex items-center justify-center gap-1.5 cursor-pointer focus:ring-0"
    >
      <StopOutline class="w-3.5 h-3.5" />
      <span>Stop Server</span>
    </Button>
  </div>
</Card>
