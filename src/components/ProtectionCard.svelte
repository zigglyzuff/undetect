<script>
  import { ShieldCheckOutline } from 'flowbite-svelte-icons';

  export let contentProtected = true;
  export let skipTaskbar = false;
  export let onToggleProtection = (val) => {};
  export let onToggleTaskbar = (val) => {};
</script>

<div class="w-full bg-zinc-900/60 border border-white/10 rounded-xl p-4 flex flex-col gap-3 shadow-xl backdrop-blur-md">
  <div class="text-sm font-semibold text-zinc-100 flex items-center gap-2">
    <ShieldCheckOutline class="w-4 h-4 text-zinc-300" />
    <span>Window Protection & Visibility</span>
  </div>

  <div class="flex flex-col gap-2.5">
    <!-- Toggle 1: Screen Capture Exclusion -->
    <button
      type="button"
      onclick={() => onToggleProtection(!contentProtected)}
      class="w-full flex items-center justify-between p-3 bg-white/[0.02] hover:bg-white/[0.05] border border-white/[0.06] hover:border-white/10 rounded-lg transition-all text-left cursor-pointer"
    >
      <div class="flex flex-col pr-3">
        <span class="text-xs font-semibold text-zinc-200">Exclude Window from Screenshots</span>
        <span class="text-[11px] text-zinc-400"></span>
      </div>
      <!-- Monochrome Switch -->
      <div class="w-10 h-6 rounded-full transition-colors relative shrink-0 p-0.5 {contentProtected ? 'bg-white' : 'bg-zinc-800 border border-zinc-700'}">
        <div class="w-5 h-5 rounded-full transition-transform {contentProtected ? 'bg-zinc-950 translate-x-4' : 'bg-zinc-400 translate-x-0'} shadow-sm"></div>
      </div>
    </button>

    <!-- Toggle 2: Hide from Taskbar -->
    <button
      type="button"
      onclick={() => onToggleTaskbar(!skipTaskbar)}
      class="w-full flex items-center justify-between p-3 bg-white/[0.02] hover:bg-white/[0.05] border border-white/[0.06] hover:border-white/10 rounded-lg transition-all text-left cursor-pointer"
    >
      <div class="flex flex-col pr-3">
        <span class="text-xs font-semibold text-zinc-200">Hide from Windows Taskbar</span>
        <span class="text-[11px] text-zinc-400"></span>
      </div>
      <!-- Monochrome Switch -->
      <div class="w-10 h-6 rounded-full transition-colors relative shrink-0 p-0.5 {skipTaskbar ? 'bg-white' : 'bg-zinc-800 border border-zinc-700'}">
        <div class="w-5 h-5 rounded-full transition-transform {skipTaskbar ? 'bg-zinc-950 translate-x-4' : 'bg-zinc-400 translate-x-0'} shadow-sm"></div>
      </div>
    </button>
  </div>
</div>
