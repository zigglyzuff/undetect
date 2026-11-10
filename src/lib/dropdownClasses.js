/**
 * Shared monochrome Flowbite dropdown styling and trigger-width sync helper.
 */

export const DROPDOWN_CLASS =
  'mt-0 divide-y-0 bg-transparent dark:bg-transparent border-0 dark:border-0 p-0 shadow-none rounded-none';

export const DROPDOWN_GROUP_CLASS =
  'm-0 p-1.5 flex flex-col gap-0.5 max-h-56 overflow-y-auto rounded-xl border border-zinc-700/90 dark:border-zinc-700/90 bg-zinc-950 dark:bg-zinc-950 shadow-2xl divide-y-0';

export function getDropdownItemClass(selected) {
  const stateClasses = selected
    ? 'bg-zinc-900 dark:bg-zinc-900 text-white dark:text-white font-semibold'
    : 'bg-transparent dark:bg-transparent text-zinc-200 dark:text-zinc-200';
  return `w-full px-2.5 py-2 rounded-lg text-xs font-sans flex items-center justify-between gap-2 cursor-pointer transition-colors hover:bg-zinc-800 dark:hover:bg-zinc-800 hover:text-white dark:hover:text-white ${stateClasses}`;
}

export function syncDropdownWidth(ev) {
  if (ev.newState === 'open' && ev.target && ev.trigger) {
    ev.target.style.width = `${ev.trigger.offsetWidth}px`;
  }
}
