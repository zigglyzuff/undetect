<script>
  import { Card, Toggle } from 'flowbite-svelte';
  import { ShieldCheckOutline } from 'flowbite-svelte-icons';

  export let contentProtected = true;
  export let skipTaskbar = false;
  export let onToggleProtection = (val) => {};
  export let onToggleTaskbar = (val) => {};

  const monoToggleClasses = {
    span: 'me-0 border-0 ring-1 ring-inset ring-zinc-700 bg-zinc-800 dark:bg-zinc-800 after:border-0 after:top-[2px] after:start-[2px] after:h-4 after:w-4 after:bg-zinc-300 peer-checked:bg-white dark:peer-checked:bg-white peer-checked:ring-white peer-checked:after:bg-zinc-950 peer-focus:ring-0 dark:peer-focus:ring-0',
  };
</script>

<Card
  size="xl"
  color="gray"
  class="w-full bg-zinc-900/60 dark:bg-zinc-900/60 border border-zinc-800 dark:border-zinc-800 rounded-xl p-4 flex flex-col gap-3 shadow-xl"
>
  <div class="text-sm font-semibold text-zinc-100 flex items-center gap-2">
    <ShieldCheckOutline class="w-4 h-4 text-zinc-300" />
    <span>Window Protection &amp; Visibility</span>
  </div>

  <div class="flex flex-col gap-2.5">
    <div class="w-full flex items-center justify-between p-3 bg-white/[0.02] border border-white/[0.06] rounded-lg">
      <div class="flex flex-col pr-3">
        <span class="text-xs font-semibold text-zinc-200">Exclude Window from Screenshots</span>
      </div>
      <Toggle
        color="gray"
        size="small"
        checked={contentProtected}
        onchange={() => onToggleProtection(!contentProtected)}
        classes={monoToggleClasses}
        aria-label="Exclude Window from Screenshots"
      />
    </div>

    <div class="w-full flex items-center justify-between p-3 bg-white/[0.02] border border-white/[0.06] rounded-lg">
      <div class="flex flex-col pr-3">
        <span class="text-xs font-semibold text-zinc-200">Hide from Windows Taskbar</span>
      </div>
      <Toggle
        color="gray"
        size="small"
        checked={skipTaskbar}
        onchange={() => onToggleTaskbar(!skipTaskbar)}
        classes={monoToggleClasses}
        aria-label="Hide from Windows Taskbar"
      />
    </div>
  </div>
</Card>
