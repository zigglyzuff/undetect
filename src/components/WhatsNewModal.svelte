<script>
  import { Badge, Button, Modal } from 'flowbite-svelte';
  import logo from '../assets/logo.png';

  export let isOpen = false;
  export let version = 'v2.1.0';
  export let onClose = () => {};
</script>

<Modal
  bind:open={isOpen}
  oncancel={onClose}
  ontoggle={(e) => {
    if (e.newState !== 'open') onClose();
  }}
  size="xs"
  class="w-[calc(100%-2rem)] max-w-[340px] mx-auto bg-zinc-950 dark:bg-zinc-950 border border-zinc-800 dark:border-zinc-800 rounded-2xl shadow-2xl text-zinc-100 dark:text-zinc-100 p-0 backdrop:bg-black/75 backdrop:backdrop-blur-xs"
  classes={{
    body: 'p-5 flex flex-col gap-3.5',
    close: 'text-zinc-400 hover:text-white bg-zinc-800/80 hover:bg-zinc-800 rounded-lg p-1 focus:ring-0',
  }}
>
  <!-- Header with Logo and Badge -->
  <div class="flex items-center gap-2.5">
    <img src={logo} alt="Undetect Logo" class="w-7 h-7 object-contain" />
    <div>
      <div class="flex items-center gap-1.5">
        <h3 class="text-sm font-bold text-white tracking-wide">What's New</h3>
        <Badge color="gray" class="text-[10px] font-mono font-semibold px-1.5 py-0 rounded bg-zinc-800 dark:bg-zinc-800 text-zinc-300 dark:text-zinc-300 border border-zinc-700 dark:border-zinc-700">
          {version}
        </Badge>
      </div>
      <p class="text-[11px] text-zinc-400 mt-0.5">AI Chat, Live Voice &amp; Interview Context</p>
    </div>
  </div>

  <!-- Feature Highlights List -->
  <div class="flex flex-col gap-2.5 bg-black/40 border border-white/5 rounded-xl p-3 text-left">
    <div class="flex items-start gap-2.5">
      <div class="w-5 h-5 rounded-md bg-zinc-800 border border-zinc-700/80 flex items-center justify-center shrink-0 mt-0.5 text-zinc-300">
        <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M8 10h.01M12 10h.01M16 10h.01M9 16H5a2 2 0 01-2-2V6a2 2 0 012-2h14a2 2 0 012 2v8a2 2 0 01-2 2h-5l-5 5v-5z"></path></svg>
      </div>
      <div class="flex flex-col">
        <span class="text-xs font-semibold text-zinc-200">AI Interview Chat</span>
        <span class="text-[11px] text-zinc-400 leading-snug">Screen-aware chat with local OCR, Groq &amp; Gemini models.</span>
      </div>
    </div>

    <div class="flex items-start gap-2.5">
      <div class="w-5 h-5 rounded-md bg-zinc-800 border border-zinc-700/80 flex items-center justify-center shrink-0 mt-0.5 text-zinc-300">
        <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M19 11a7 7 0 01-7 7m0 0a7 7 0 01-7-7m7 7v4m0 0H8m4 0h4m-4-8a3 3 0 01-3-3V5a3 3 0 116 0v6a3 3 0 01-3 3z"></path></svg>
      </div>
      <div class="flex flex-col">
        <span class="text-xs font-semibold text-zinc-200">Live Voice Transcription</span>
        <span class="text-[11px] text-zinc-400 leading-snug">Real-time Deepgram or offline Vosk capture for mic &amp; system audio.</span>
      </div>
    </div>

    <div class="flex items-start gap-2.5">
      <div class="w-5 h-5 rounded-md bg-zinc-800 border border-zinc-700/80 flex items-center justify-center shrink-0 mt-0.5 text-zinc-300">
        <svg class="w-3 h-3" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"></path></svg>
      </div>
      <div class="flex flex-col">
        <span class="text-xs font-semibold text-zinc-200">Resume &amp; JD Context</span>
        <span class="text-[11px] text-zinc-400 leading-snug">Upload your resume in Settings and add a job description in Chat.</span>
      </div>
    </div>
  </div>

  <!-- Action Button -->
  <Button
    color="alternative"
    size="xs"
    onclick={onClose}
    class="w-full py-2.5 px-4 rounded-xl bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none text-xs font-semibold transition-all shadow cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
  >
    <span class="text-zinc-950">Got it, let's go</span>
  </Button>
</Modal>
