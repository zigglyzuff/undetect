<script>
  import logo from '../../assets/logo.png';

  export let count = 0;
  export let currentIndex = -1;
  export let currentTimestamp = '';
  export let status = 'online'; // 'online' | 'locked' | 'connecting'
</script>

<nav class="w-full bg-zinc-950 border-b border-white/10 px-4 py-2.5 shrink-0 z-40 select-none">
  <div class="w-full flex items-center justify-between gap-3">
    <!-- Brand Logo, Name & Refined Non-Cliché Live Indicator -->
    <div class="flex items-center gap-3">
      <div class="flex items-center gap-2">
        <img src={logo} alt="Undetect Logo" class="w-5 h-5 object-contain" />
        <span class="font-bold text-sm tracking-wider text-white font-mono uppercase">Undetect</span>
      </div>

      <!-- Understated Modern Live Status Pill -->
      <div class="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-zinc-900 border border-white/5 text-[10px] font-mono tracking-wider">
        {#if status === 'online'}
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.6)]"></span>
          <span class="text-zinc-300 font-semibold uppercase">Live</span>
        {:else if status === 'connecting'}
          <span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse"></span>
          <span class="text-zinc-400 uppercase">Connecting</span>
        {:else}
          <span class="w-1.5 h-1.5 rounded-full bg-zinc-600"></span>
          <span class="text-zinc-500 uppercase">Locked</span>
        {/if}
      </div>
    </div>

    <!-- Active Capture Metadata in Nav -->
    <div class="flex items-center gap-2 text-xs font-mono">
      {#if currentTimestamp}
        <span class="text-zinc-400 hidden sm:inline">Captured:</span>
        <span class="text-zinc-200 font-semibold">{currentTimestamp}</span>
        <span class="text-zinc-500 font-normal">({currentIndex + 1}/{count})</span>
      {:else}
        <span class="text-zinc-500 font-normal">No captures</span>
      {/if}
    </div>
  </div>
</nav>
