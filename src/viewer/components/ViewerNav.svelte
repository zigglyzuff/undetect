<script>
  import logo from '../../assets/logo.png';


  export let count = 0;
  export let currentIndex = -1;
  export let currentTimestamp = '';
  export let status = 'online'; // 'online' | 'locked' | 'connecting'
</script>

<nav class="w-full bg-zinc-950 border-b border-zinc-800 px-4 py-2.5 shrink-0 z-40 select-none">
  <div class="w-full flex items-center justify-between gap-3">
    <!-- Brand Logo, Name & Refined Monochrome Indicator -->
    <div class="flex items-center gap-3">
      <div class="flex items-center gap-2">
        <img src={logo} alt="Undetect Logo" class="w-5 h-5 object-contain" />
        <span class="font-bold text-sm tracking-wider text-white font-mono uppercase">Undetect</span>
      </div>

      <!-- Understated Monochrome Live Status Pill -->
      <div class="flex items-center gap-1.5 px-2 py-0.5 rounded-full bg-zinc-900 border border-zinc-800 text-[10px] font-mono tracking-wider">
        {#if status === 'online'}
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.65)]"></span>
          <span class="text-zinc-200 font-semibold uppercase">Live</span>
        {:else if status === 'connecting'}
          <span class="w-1.5 h-1.5 rounded-full bg-zinc-400 animate-pulse"></span>
          <span class="text-zinc-400 uppercase">Connecting</span>
        {:else}
          <span class="w-1.5 h-1.5 rounded-full bg-zinc-600"></span>
          <span class="text-zinc-500 uppercase">Locked</span>
        {/if}
      </div>
    </div>

    <!-- Active Capture Metadata -->
    <div class="flex items-center gap-2.5 text-xs font-mono">
      {#if currentTimestamp}
        <span class="text-zinc-400 hidden sm:inline">Captured:</span>
        <span class="text-zinc-200 font-semibold hidden sm:inline">{currentTimestamp}</span>
        <span class="text-zinc-500 font-normal hidden sm:inline">({currentIndex + 1}/{count})</span>
      {/if}
    </div>
  </div>
</nav>
