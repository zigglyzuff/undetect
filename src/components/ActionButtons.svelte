<script>
  import { CameraPhotoOutline, TrashBinOutline } from 'flowbite-svelte-icons';

  export let count = 0;
  export let onCapture = () => {};
  export let onClear = () => {};
</script>

<div class="w-full grid grid-cols-2 gap-2.5 pt-1">
  <button
    type="button"
    onclick={onCapture}
    class="w-full py-2.5 px-3 bg-zinc-900 hover:bg-zinc-800 active:bg-zinc-750 border border-zinc-800 hover:border-zinc-700 text-zinc-100 rounded-lg text-xs font-semibold flex items-center justify-center gap-2 cursor-pointer shadow-sm transition-all active:scale-98"
  >
    <CameraPhotoOutline class="w-4 h-4 text-zinc-300" />
    <span>Capture Now</span>
  </button>

  <button
    type="button"
    onclick={onClear}
    class="w-full py-2.5 px-3 bg-zinc-900 hover:bg-rose-950/40 active:bg-rose-950/60 border border-zinc-800 hover:border-rose-900/50 text-zinc-400 hover:text-rose-300 rounded-lg text-xs font-semibold flex items-center justify-center gap-2 cursor-pointer shadow-sm transition-all active:scale-98"
  >
    <TrashBinOutline class="w-4 h-4" />
    <span>Clear</span>
    <span class="ml-1 text-[10px] font-mono px-1.5 py-0.2 rounded bg-zinc-800 text-zinc-300 border border-zinc-700">
      {count}
    </span>
  </button>
</div>
