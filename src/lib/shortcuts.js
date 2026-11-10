/**
 * Clear, one-binding-per-action keyboard shortcuts grouped by scope.
 */

export const GLOBAL_SHORTCUTS = [
  { keys: ['Ctrl', 'Alt', 'H'], label: 'Show or hide window' },
  { keys: ['Ctrl', 'Alt', 'K'], label: 'Focus chat input' },
  { keys: ['Ctrl', 'Alt', 'Enter'], label: 'Answer current screen directly' },
  { keys: ['Ctrl', 'Alt', 'V'], label: 'Start or stop voice input' },
  { keys: ['Ctrl', 'Alt', 'A'], label: 'Include or exclude screen capture' },
  { keys: ['Ctrl', 'Alt', 'G'], label: 'Toggle click-through Ghost Mode' },
];

export const IN_APP_SHORTCUTS = [
  { keys: ['Enter'], label: 'Send message' },
  { keys: ['Shift', 'Enter'], label: 'New line' },
  { keys: ['Alt', 'Enter'], label: 'Send voice transcript' },
  { keys: ['Alt', 'Backspace'], label: 'Clear voice transcript' },
  { keys: ['Ctrl', 'J'], label: 'Add or edit job description' },
  { keys: ['Ctrl', '↑ / ↓'], label: 'Scroll chat messages' },
  { keys: ['Ctrl', '[ / ]'], label: 'Resize window width' },
  { keys: ['Ctrl', ','], label: 'Open or close Settings' },
];

export const MORE_SHORTCUTS = [
  { keys: ['Ctrl', 'Alt', 'Shift', '↑ / ↓'], label: 'Scroll chat from any app' },
  { keys: ['Ctrl', 'Alt', 'Shift', '← / →'], label: 'Resize width from any app' },
  { keys: ['Ctrl', 'Alt', 'P'], label: 'Toggle screenshot privacy' },
  { keys: ['Ctrl', 'Alt', '0'], label: 'Reset window size' },
];

export const WINDOW_SIZE_PRESETS = [
  { id: 'compact', label: 'Compact', width: 380, height: 560 },
  { id: 'default', label: 'Default', width: 460, height: 710 },
  { id: 'wide', label: 'Wide', width: 640, height: 760 },
];
