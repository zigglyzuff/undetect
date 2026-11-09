<script>
  export let message = '';
  export let visible = false;
</script>

{#if visible}
  <div class="fixed top-13 left-1/2 -translate-x-1/2 z-50 pointer-events-none transition-all duration-300 animate-in fade-in slide-in-from-top-2">
    <div class="bg-zinc-900/95 border border-zinc-700/80 text-zinc-100 shadow-2xl py-1.5 px-4 rounded-full text-xs font-medium flex items-center gap-2.5 backdrop-blur-xl whitespace-nowrap select-none">
      <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 shadow-[0_0_8px_#34d399] shrink-0"></span>
      <span class="text-zinc-200">{message}</span>
    </div>
  </div>
{/if}
