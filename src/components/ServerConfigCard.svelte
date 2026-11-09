<script>
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

<div class="w-full bg-zinc-900/60 border border-white/10 rounded-xl p-4 flex flex-col gap-3 shadow-xl backdrop-blur-md">
  <div class="flex items-center justify-between">
    <span class="text-sm font-semibold text-zinc-100">Server Configuration</span>
    {#if running}
      <span class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-semibold bg-zinc-800 text-emerald-400 border border-emerald-500/30">
        <span class="w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_8px_#34d399] animate-pulse"></span>
        Active
      </span>
    {:else}
      <span class="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-xs font-semibold bg-zinc-800 text-zinc-400 border border-zinc-700">
        <span class="w-2 h-2 rounded-full bg-zinc-500"></span>
        Stopped
      </span>
    {/if}
  </div>

  <div class="flex flex-col gap-1.5">
    <label for="server-pin" class="text-xs font-medium text-zinc-400">Access Password (PIN)</label>
    <input
      id="server-pin"
      type="password"
      bind:value={inputPassword}
      placeholder="Set Passcode"
      class="w-full px-3 py-2 bg-black/50 border border-zinc-800 rounded-lg text-zinc-100 text-sm focus:outline-none focus:border-white focus:ring-1 focus:ring-white transition-colors"
    />
  </div>

  <div class="flex flex-col gap-1.5">
    <label for="server-port" class="text-xs font-medium text-zinc-400">Port</label>
    <input
      id="server-port"
      type="number"
      bind:value={inputPort}
      placeholder="9090"
      class="w-full px-3 py-2 bg-black/50 border border-zinc-800 rounded-lg text-zinc-100 text-sm focus:outline-none focus:border-white focus:ring-1 focus:ring-white transition-colors"
    />
  </div>

  <div class="grid grid-cols-2 gap-2.5 pt-1">
    <button
      type="button"
      onclick={handleStart}
      class="w-full px-3 py-2 bg-white hover:bg-zinc-200 active:bg-zinc-300 text-zinc-950 rounded-lg text-xs font-semibold transition-all shadow-md flex items-center justify-center gap-1.5 cursor-pointer active:scale-98"
    >
      <PlayOutline class="w-3.5 h-3.5 text-zinc-950" />
      <span>Update & Start</span>
    </button>
    <button
      type="button"
      onclick={onStop}
      class="w-full px-3 py-2 bg-zinc-900 hover:bg-rose-950/40 text-zinc-400 hover:text-rose-300 border border-zinc-800 hover:border-rose-900/50 rounded-lg text-xs font-semibold transition-all flex items-center justify-center gap-1.5 cursor-pointer active:scale-98"
    >
      <StopOutline class="w-3.5 h-3.5" />
      <span>Stop Server</span>
    </button>
  </div>
</div>
