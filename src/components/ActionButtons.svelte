<script>
  import { Badge, Button } from 'flowbite-svelte';
  import { CameraPhotoOutline, TrashBinOutline } from 'flowbite-svelte-icons';

  export let count = 0;
  export let onCapture = () => {};
  export let onClear = () => {};
</script>

<div class="w-full grid grid-cols-2 gap-2.5 pt-1">
  <Button
    color="alternative"
    size="xs"
    onclick={onCapture}
    class="w-full py-2.5 px-3 bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 dark:hover:bg-zinc-800 border-zinc-800 dark:border-zinc-800 hover:border-zinc-700 text-zinc-100 dark:text-zinc-100 rounded-lg text-xs font-semibold flex items-center justify-center gap-2 cursor-pointer shadow-sm transition-all focus:ring-0"
  >
    <CameraPhotoOutline class="w-4 h-4 text-zinc-300" />
    <span>Capture Now</span>
  </Button>

  <Button
    color="alternative"
    size="xs"
    onclick={onClear}
    class="w-full py-2.5 px-3 bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 dark:hover:bg-zinc-800 border-zinc-800 dark:border-zinc-800 hover:border-zinc-700 text-zinc-400 dark:text-zinc-400 hover:text-white rounded-lg text-xs font-semibold flex items-center justify-center gap-2 cursor-pointer shadow-sm transition-all focus:ring-0"
  >
    <TrashBinOutline class="w-4 h-4" />
    <span>Clear</span>
    <Badge color="gray" class="ml-1 text-[10px] font-mono px-1.5 py-0 rounded bg-zinc-800 dark:bg-zinc-800 text-zinc-300 dark:text-zinc-300 border border-zinc-700 dark:border-zinc-700">
      {count}
    </Badge>
  </Button>
</div>
