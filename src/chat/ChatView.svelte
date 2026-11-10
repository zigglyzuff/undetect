<script>
  import { onMount, onDestroy, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { Button, Modal, Textarea, Tooltip } from 'flowbite-svelte';
  import {
    BriefcaseOutline,
    DesktopPcOutline,
    PaperPlaneOutline,
    TrashBinOutline,
    CameraPhotoOutline,
    MicrophoneOutline,
    MicrophoneSolid
  } from 'flowbite-svelte-icons';

  import { ChatMessage } from './models/ChatMessage.js';
  import { ChatService, isLlmConfigured } from './services/ChatService.js';
  import { formatMarkdown } from './utils/markdown.js';
  import { TOOLTIP_CLASS } from '../lib/tooltip.js';

  export let currentScreenshotId = null;
  export let authPin = '';
  export let onOpenSettings = () => {};
  export let showToast = () => {};

  const chatService = new ChatService({ authPin });

  let messages = [];
  let inputText = '';
  let isLoading = false;
  export let attachScreenshot = true;

  let configLoaded = false;
  let llmConfig = { active_provider: 'groq', has_gemini_key: false, has_groq_key: false };
  let voiceLoaded = false;
  let voiceReady = true;

  let messagesContainer;
  let textareaRef;
  let jdTextareaRef;

  let jdModalOpen = false;
  let jdDraft = '';
  let savingJd = false;
  let jdError = '';

  const MODAL_CLASS =
    'w-[calc(100%-2rem)] max-w-sm mx-auto bg-zinc-950 dark:bg-zinc-950 border border-zinc-800 dark:border-zinc-800 text-zinc-100 dark:text-zinc-100 divide-y divide-zinc-800/80 dark:divide-zinc-800/80 rounded-2xl shadow-2xl p-0 backdrop:bg-black/75 backdrop:backdrop-blur-xs';
  const MODAL_CLASSES = {
    header: 'bg-zinc-950 dark:bg-zinc-950 border-b border-zinc-800/80 dark:border-zinc-800/80 text-white px-4 py-3 text-sm font-semibold rounded-t-2xl',
    body: 'bg-zinc-950 dark:bg-zinc-950 px-4 py-3.5 space-y-2.5 text-xs',
    footer: 'bg-zinc-950 dark:bg-zinc-950 border-t border-zinc-800/80 dark:border-zinc-800/80 px-4 py-2.5 rounded-b-2xl',
    close: 'text-zinc-400 hover:text-white hover:bg-zinc-800 rounded-lg p-1 focus:ring-0',
  };

  let isListening = false;
  let voiceProvider = 'deepgram';
  let voiceSource = 'system';
  let voiceAutoSend = false;
  let manualVoiceStop = false;
  let voiceFinalText = '';
  let voicePartialText = '';
  let llmSetupModalOpen = false;
  let voiceSetupModalOpen = false;
  let voiceSetupProvider = 'deepgram';
  let installingVoice = false;
  let voiceSetupProgress = { percent: 0, message: '' };
  let voiceSetupError = '';
  let voiceUnlistenFns = [];

  $: liveVoiceText = mergeVoiceText(voiceFinalText, voicePartialText);
  $: showVoiceBubble = isListening || liveVoiceText.length > 0;
  $: activeLlmLabel = llmConfig.active_provider === 'groq' ? 'Groq' : 'Gemini';
  $: llmReady = isLlmConfigured(llmConfig);
  $: hasJd = Boolean(llmConfig?.has_jd);
  $: savedJd = llmConfig?.job_description || '';
  $: showLlmMissing = configLoaded && !llmReady;
  $: showVoiceMissing = voiceLoaded && !voiceReady;

  export async function openJdModal() {
    jdDraft = savedJd;
    jdError = '';
    jdModalOpen = true;
    await tick();
    requestAnimationFrame(() => jdTextareaRef?.focus());
  }

  async function saveJdValue(text, successMsg) {
    if (savingJd) return;
    savingJd = true;
    jdError = '';
    try {
      const updated = await chatService.saveJobDescription(text);
      if (updated) {
        llmConfig = { ...llmConfig, ...updated };
      }
      jdModalOpen = false;
      showToast(successMsg);
    } catch (err) {
      jdError = err?.message || String(err);
    } finally {
      savingJd = false;
    }
  }

  function handleSaveJd() {
    return saveJdValue(jdDraft, jdDraft.trim() ? 'Job description saved to chat context' : 'Job description cleared');
  }

  function handleClearJd() {
    jdDraft = '';
    return saveJdValue('', 'Job description cleared');
  }

  export async function refreshAvailability() {
    const [cfgRes, voiceRes] = await Promise.allSettled([
      chatService.getConfig(),
      invoke('get_voice_status'),
    ]);

    if (cfgRes.status === 'fulfilled' && cfgRes.value) {
      llmConfig = cfgRes.value;
      configLoaded = true;
    }

    if (voiceRes.status === 'fulfilled' && voiceRes.value) {
      const s = voiceRes.value;
      isListening = Boolean(s.listening);
      voiceProvider = s.provider || 'deepgram';
      voiceSource = s.source || 'system';
      voiceAutoSend = Boolean(s.auto_send);
      voiceReady = Boolean(s.ready);
      voiceLoaded = true;
    }
  }

  function mergeVoiceText(base, chunk) {
    const cleanChunk = (chunk || '').trim();
    if (!cleanChunk) return base;
    if (!base) return cleanChunk;
    const spacer = /\s$/.test(base) ? '' : ' ';
    return `${base}${spacer}${cleanChunk}`;
  }

  function clearVoiceBubble() {
    voiceFinalText = '';
    voicePartialText = '';
  }

  export function clearVoiceTranscript() {
    if (!liveVoiceText) return false;
    clearVoiceBubble();
    return true;
  }

  async function handleSendVoiceTranscript() {
    const text = liveVoiceText.trim();
    if (!text || isLoading) return;
    if (configLoaded && !llmReady) {
      llmSetupModalOpen = true;
      return;
    }
    if (isListening) {
      await invoke('stop_voice_input').catch(() => {});
    }
    clearVoiceBubble();
    await handleSendMessage(text);
  }

  export function sendVoiceTranscript() {
    return handleSendVoiceTranscript();
  }

  async function handleToggleVoice() {
    try {
      const status = await invoke('get_voice_status');
      voiceProvider = status?.provider || 'deepgram';
      voiceSource = status?.source || 'system';
      voiceAutoSend = Boolean(status?.auto_send);
      voiceReady = Boolean(status?.ready);
      voiceLoaded = true;
      if (status?.listening) {
        manualVoiceStop = true;
        await invoke('stop_voice_input');
        return;
      }
      manualVoiceStop = false;
      if (!status?.ready) {
        voiceSetupError = '';
        voiceSetupProvider = voiceProvider;
        voiceSetupModalOpen = true;
        showToast(
          voiceProvider === 'deepgram'
            ? "Voice input won't work until a Deepgram API key is added."
            : "Voice input won't work until Vosk or Deepgram is configured."
        );
        return;
      }
      await invoke('start_voice_input');
    } catch (err) {
      isListening = false;
      voiceSetupError = String(err || 'Voice input failed');
    }
  }

  async function handleSwitchVoiceSource() {
    const nextSource = voiceSource === 'system' ? 'mic' : 'system';
    try {
      const updated = await invoke('set_voice_source', { source: nextSource });
      if (updated) {
        voiceSource = updated.source || nextSource;
        voiceProvider = updated.provider || voiceProvider;
        voiceAutoSend = Boolean(updated.auto_send);
        voiceReady = Boolean(updated.ready);
      } else {
        voiceSource = nextSource;
      }
      showToast(voiceSource === 'system' ? 'Voice source: System Audio (Interviewer)' : 'Voice source: Microphone');
    } catch (err) {
      showToast('Failed to switch audio source: ' + err);
    }
  }

  async function handleInstallAndStartVoice() {
    if (installingVoice) return;
    installingVoice = true;
    voiceSetupError = '';
    voiceSetupProgress = { percent: 4, message: 'Starting offline voice setup...' };
    try {
      const res = await invoke('setup_vosk');
      if (res) {
        voiceReady = Boolean(res.ready);
        voiceLoaded = true;
      }
      voiceSetupModalOpen = false;
      await invoke('start_voice_input');
    } catch (err) {
      voiceSetupError = String(err || 'Failed to download offline voice engine');
    } finally {
      installingVoice = false;
    }
  }

  export function focusInput() {
    textareaRef?.focus();
  }

  export function toggleScreenAttachment() {
    attachScreenshot = !attachScreenshot;
    return attachScreenshot;
  }

  export function scrollMessages(deltaY) {
    if (messagesContainer) {
      messagesContainer.scrollTop += deltaY;
    }
  }

  export async function copyLatestCodeOrReply() {
    const codeBtns = messagesContainer?.querySelectorAll('.copy-btn');
    if (codeBtns && codeBtns.length > 0) {
      const lastBtn = codeBtns[codeBtns.length - 1];
      const encoded = lastBtn.getAttribute('data-code');
      const code = encoded
        ? decodeURIComponent(encoded)
        : (lastBtn.closest('.code-block')?.querySelector('pre code')?.textContent || '');
      if (code) {
        await navigator.clipboard.writeText(code);
        return 'code';
      }
    }

    const lastAssistant = [...messages].reverse().find((m) => !m.isUser() && !m.isPending && m.content);
    if (lastAssistant) {
      await navigator.clipboard.writeText(lastAssistant.content);
      return 'reply';
    }
    return null;
  }

  export function clearChatImmediately() {
    if (messages.length === 0) return false;
    messages = [];
    sessionStorage.removeItem('undetect_chat_history');
    return true;
  }

  export function answerScreen() {
    return handleSendMessage('Answer the question or problem currently shown on my screen concisely.', {
      forceScreenshot: true,
    });
  }

  $: if (authPin) {
    chatService.setAuthPin(authPin);
  }

  onMount(async () => {
    loadStoredHistory();
    await refreshAvailability();
    window.addEventListener('undetect:config-updated', refreshAvailability);

    try {
      voiceUnlistenFns = await Promise.all([
        listen('vosk://transcript', ({ payload }) => {
          if (!payload || !payload.text) return;
          if (payload.kind === 'partial') {
            voicePartialText = payload.text;
          } else if (payload.kind === 'final') {
            voiceFinalText = mergeVoiceText(voiceFinalText, payload.text);
            voicePartialText = '';
          }
        }),
        listen('vosk://state', ({ payload }) => {
          const wasListening = isListening;
          isListening = Boolean(payload);
          if (!isListening) {
            if (voicePartialText.trim()) {
              voiceFinalText = mergeVoiceText(voiceFinalText, voicePartialText);
              voicePartialText = '';
            }
            const transcriptReady = mergeVoiceText(voiceFinalText, voicePartialText).trim();
            const shouldAutoSend = wasListening && voiceAutoSend && !manualVoiceStop && transcriptReady.length > 0 && !isLoading;
            manualVoiceStop = false;
            if (shouldAutoSend) {
              handleSendVoiceTranscript();
            }
          }
        }),
        listen('vosk://setup-progress', ({ payload }) => {
          if (payload) voiceSetupProgress = payload;
        }),
        listen('shortcut://voice-unconfigured', ({ payload }) => {
          voiceProvider = payload?.provider || voiceProvider;
          voiceReady = false;
          voiceLoaded = true;
          voiceSetupError = '';
          voiceSetupProvider = voiceProvider;
          voiceSetupModalOpen = true;
          showToast(
            voiceProvider === 'deepgram'
              ? "Voice input won't work until a Deepgram API key is added."
              : "Voice input won't work until Vosk or Deepgram is configured."
          );
        }),
      ]);
    } catch (e) {}
  });

  onDestroy(() => {
    window.removeEventListener('undetect:config-updated', refreshAvailability);
    for (const unlisten of voiceUnlistenFns) {
      if (typeof unlisten === 'function') unlisten();
    }
  });

  function loadStoredHistory() {
    try {
      const saved = sessionStorage.getItem('undetect_chat_history');
      if (saved) {
        const parsed = JSON.parse(saved);
        messages = parsed.map((item) => new ChatMessage(item));
      }
    } catch (e) {}
  }

  function persistHistory() {
    try {
      sessionStorage.setItem('undetect_chat_history', JSON.stringify(messages));
    } catch (e) {}
  }

  function scrollToBottom() {
    tick().then(() => {
      if (messagesContainer) {
        messagesContainer.scrollTop = messagesContainer.scrollHeight;
      }
    });
  }

  function scrollMessageTopIntoView(messageId) {
    tick().then(() => {
      requestAnimationFrame(() => {
        if (!messagesContainer) return;
        const targetEl = document.getElementById(`chat-msg-${messageId}`);
        if (!targetEl) return;
        const containerRect = messagesContainer.getBoundingClientRect();
        const targetRect = targetEl.getBoundingClientRect();
        messagesContainer.scrollTop += targetRect.top - containerRect.top - 8;
      });
    });
  }

  async function handleSendMessage(promptOverride = null, options = {}) {
    const text = (promptOverride || inputText).trim();
    if (!text || isLoading) return;

    if (configLoaded && !llmReady) {
      llmSetupModalOpen = true;
      return;
    }

    const useScreen = Boolean(options?.forceScreenshot) || attachScreenshot;
    const resolvedScreenshotId = useScreen ? (currentScreenshotId || 'capture') : null;

    const userMessage = new ChatMessage({
      role: 'user',
      content: text,
      screenshotId: resolvedScreenshotId,
    });

    messages = [...messages, userMessage];
    if (!promptOverride) {
      inputText = '';
    }
    isLoading = true;
    scrollToBottom();

    const pendingMsg = new ChatMessage({
      role: 'assistant',
      content: '',
      isPending: true,
    });
    messages = [...messages, pendingMsg];
    scrollToBottom();

    let anchoredTop = false;
    let streamedText = '';

    try {
      // Provider, model, and OCR behavior are configured under Settings -> Model Provider
      const response = await chatService.sendChatStream({
        messages: messages.filter((m) => !m.isPending && (m.id !== pendingMsg.id)),
        screenshotId: resolvedScreenshotId,
        useOcr: false,
        provider: null,
        model: null,
        onChunk: (delta) => {
          streamedText += delta;
          messages = messages.map((m) =>
            m.id === pendingMsg.id
              ? new ChatMessage({
                  id: m.id,
                  role: 'assistant',
                  content: streamedText,
                  isPending: false,
                })
              : m
          );
          if (!anchoredTop) {
            anchoredTop = true;
            scrollMessageTopIntoView(pendingMsg.id);
          }
        },
      });

      messages = messages.map((m) => {
        if (m.id === pendingMsg.id) {
          return new ChatMessage({
            id: m.id,
            role: 'assistant',
            content: response.reply || streamedText,
            isPending: false,
          });
        }
        return m;
      });

      persistHistory();
      if (!anchoredTop) {
        scrollMessageTopIntoView(pendingMsg.id);
      }
    } catch (err) {
      messages = messages.map((m) => {
        if (m.id === pendingMsg.id) {
          return new ChatMessage({
            id: m.id,
            role: 'assistant',
            content: `**Error**: ${err.message || err}`,
            error: err.message || err,
            isPending: false,
          });
        }
        return m;
      });
      scrollMessageTopIntoView(pendingMsg.id);
    } finally {
      isLoading = false;
    }
  }

  function handleKeydown(e) {
    if (e.key === 'Enter' && e.altKey) {
      e.preventDefault();
      if (liveVoiceText.trim()) {
        handleSendVoiceTranscript();
      }
      return;
    }
    if (e.key === 'Enter' && !e.shiftKey && !e.ctrlKey && !e.metaKey) {
      e.preventDefault();
      if (inputText.trim()) {
        handleSendMessage();
      } else if (liveVoiceText.trim()) {
        handleSendVoiceTranscript();
      }
    }
  }

  function handleClearChat() {
    if (confirm('Clear chat conversation?')) {
      messages = [];
      sessionStorage.removeItem('undetect_chat_history');
    }
  }

  function handleContainerClick(e) {
    const btn = e.target.closest('.copy-btn');
    if (btn) {
      const encoded = btn.getAttribute('data-code');
      const code = encoded
        ? decodeURIComponent(encoded)
        : (btn.closest('.code-block')?.querySelector('pre code')?.textContent || '');
      navigator.clipboard.writeText(code).then(() => {
        const orig = btn.innerText;
        btn.innerText = 'Copied!';
        setTimeout(() => {
          btn.innerText = orig;
        }, 1500);
      });
    }
  }
</script>

{#snippet setupNoticeCard(centeredHero = false)}
  <div
    class="rounded-2xl bg-zinc-900/90 border border-zinc-700/80 shadow-xl select-none flex flex-col gap-3 {centeredHero
      ? 'w-full max-w-sm p-4 text-left'
      : 'mx-auto w-[calc(100%-1.5rem)] max-w-md mt-2.5 px-3.5 py-3 shrink-0'}"
  >
    {#if centeredHero}
      <div class="pb-2 border-b border-zinc-800/80 flex items-center justify-between gap-2">
        <span class="text-xs font-semibold text-white tracking-wide">Configuration Required</span>
        <span class="text-[10.5px] font-medium text-zinc-400">API Setup</span>
      </div>
    {/if}

    {#if showLlmMissing}
      <div class="flex items-center justify-between gap-3 text-xs">
        <div class="min-w-0">
          <div class="font-semibold text-zinc-100">AI Chat ({activeLlmLabel})</div>
          <p class="text-[11.5px] text-zinc-400 leading-snug mt-0.5">
            AI Chat will not work until a {activeLlmLabel} API key is added.
          </p>
        </div>
        <Button
          color="alternative"
          size="xs"
          onclick={() => onOpenSettings('model')}
          class="h-7 px-3 py-0 rounded-lg bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none text-[11px] font-semibold shrink-0 cursor-pointer focus:ring-0"
        >
          <span class="text-zinc-950">Add Key</span>
        </Button>
      </div>
    {/if}

    {#if showVoiceMissing}
      <div class="flex items-center justify-between gap-3 text-xs {showLlmMissing ? 'pt-2.5 border-t border-zinc-800/80' : ''}">
        <div class="min-w-0">
          <div class="font-semibold text-zinc-100">Voice Input</div>
          <p class="text-[11.5px] text-zinc-400 leading-snug mt-0.5">
            Voice Input will not work until {voiceProvider === 'deepgram' ? 'a Deepgram API key' : 'Vosk or Deepgram'} is configured.
          </p>
        </div>
        <Button
          color="alternative"
          size="xs"
          onclick={() => onOpenSettings('voice')}
          class="h-7 px-3 py-0 rounded-lg bg-zinc-800 dark:bg-zinc-800 hover:bg-zinc-700 dark:hover:bg-zinc-700 text-zinc-100 dark:text-zinc-100 hover:text-white border border-zinc-700 dark:border-zinc-700 text-[11px] font-medium shrink-0 cursor-pointer focus:ring-0"
        >
          Configure
        </Button>
      </div>
    {/if}
  </div>
{/snippet}

<div class="flex-1 w-full h-full flex flex-col overflow-hidden bg-zinc-950 text-zinc-100 select-none">
  {#if messages.length > 0}
    <div class="px-4 pt-2 pb-1 bg-zinc-950 flex items-center justify-end shrink-0">
      <Button
        color="alternative"
        size="xs"
        class="h-7 px-2.5 py-0 rounded-lg text-[11px] font-medium leading-none text-zinc-400 dark:text-zinc-400 hover:text-white dark:hover:text-white bg-zinc-900 dark:bg-zinc-900 border-zinc-800 dark:border-zinc-800 hover:bg-zinc-800 dark:hover:bg-zinc-800 inline-flex items-center gap-1.5 cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
        title="Clear Chat History"
        onclick={handleClearChat}
      >
        <TrashBinOutline class="w-3 h-3 shrink-0" />
        <span class="leading-none">Clear</span>
      </Button>
    </div>
  {/if}

  {#if messages.length > 0 && (showLlmMissing || showVoiceMissing)}
    {@render setupNoticeCard(false)}
  {/if}

  <!-- Messages Feed -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    data-scroll-feed
    class="flex-1 overflow-y-auto px-3 py-3 space-y-3 select-text"
    bind:this={messagesContainer}
    onclick={handleContainerClick}
  >
    {#if messages.length === 0}
      <div class="h-full flex items-center justify-center p-4 select-none">
        {#if showLlmMissing || showVoiceMissing}
          {@render setupNoticeCard(true)}
        {:else}
          <p class="text-xs text-zinc-500 text-center">Ask anything to get started.</p>
        {/if}
      </div>
    {/if}

    {#each messages as msg (msg.id)}
      <div id="chat-msg-{msg.id}" class="flex flex-col w-full">
        <div class="flex items-center justify-between w-full mb-1 px-0.5 text-[10px] font-mono text-zinc-500">
          {#if msg.isUser()}
            <span class="text-zinc-400 font-semibold">You</span>
          {:else}
            <span class="text-zinc-200 font-semibold">Assistant</span>
          {/if}
          <span>{msg.timestamp}</span>
        </div>

        <div
          class="w-full min-w-0 overflow-hidden rounded-xl px-3.5 py-2.5 text-[13px] {msg.isUser()
            ? 'bg-zinc-100 text-zinc-950 shadow-sm font-sans font-medium'
            : 'bg-zinc-900 border border-zinc-800 text-zinc-200 shadow-sm'}"
        >
          {#if msg.isPending}
            <div class="flex items-center gap-2 py-1 text-zinc-400">
              <span class="w-1.5 h-1.5 rounded-full bg-white animate-bounce"></span>
              <span class="w-1.5 h-1.5 rounded-full bg-white animate-bounce [animation-delay:0.2s]"></span>
              <span class="w-1.5 h-1.5 rounded-full bg-white animate-bounce [animation-delay:0.4s]"></span>
              <span class="text-xs ml-1 text-zinc-300">Thinking...</span>
            </div>
          {:else if msg.isUser()}
            <p class="whitespace-pre-wrap leading-relaxed">{msg.content}</p>
            {#if msg.hasScreenshot()}
              <div class="mt-2 pt-1.5 border-t border-zinc-300 flex items-center gap-1.5 text-[10px] leading-none text-zinc-600">
                <CameraPhotoOutline class="w-3 h-3 text-zinc-700 shrink-0" />
                <span>Screen included</span>
              </div>
            {/if}
          {:else}
            <div class="w-full min-w-0 leading-relaxed space-y-1">
              {@html formatMarkdown(msg.content)}
            </div>
          {/if}
        </div>
      </div>
    {/each}
  </div>

  <!-- Clean Monochrome Bottom Area: Optional Live Voice Bubble Above Input -->
  <div class="p-3 bg-zinc-950 border-t border-zinc-800 shrink-0 select-none">
    {#if showVoiceBubble}
      <div
        id="voice-transcript-bubble"
        class="mb-2.5 rounded-2xl bg-zinc-900/95 border {isListening ? 'border-emerald-500/45' : 'border-zinc-700/80'} px-3.5 py-2.5 shadow-lg transition-colors"
      >
        <div class="flex items-center justify-between gap-2 pb-1.5 border-b border-zinc-800/80">
          <div class="flex items-center gap-2 text-[11px] font-medium text-zinc-300">
            <span class="w-1.5 h-1.5 rounded-full {isListening ? 'bg-emerald-400 animate-pulse' : 'bg-zinc-500'}"></span>
            <span>{isListening ? 'Listening' : 'Voice transcript'}</span>
            <span class="px-1.5 py-0.5 rounded bg-zinc-800 text-zinc-400 text-[10px]">
              {voiceProvider === 'deepgram' ? 'Deepgram' : 'Vosk'}
            </span>
          </div>

          <div class="flex items-center gap-1.5">
            {#if liveVoiceText}
              <Button
                color="alternative"
                size="xs"
                onclick={clearVoiceBubble}
                title="Clear transcript (Alt + Backspace)"
                class="h-6 px-2 py-0 rounded-lg text-[11px] font-medium bg-zinc-800/80 dark:bg-zinc-800/80 hover:bg-zinc-700 dark:hover:bg-zinc-700 border-zinc-700/70 dark:border-zinc-700/70 text-zinc-400 dark:text-zinc-400 hover:text-white cursor-pointer focus:ring-0"
              >
                Clear
              </Button>
            {/if}

            <Button
              color="alternative"
              size="xs"
              disabled={!liveVoiceText || isLoading}
              onclick={handleSendVoiceTranscript}
              title="Send voice transcript (Alt + Enter)"
              class="h-6 px-2.5 py-0 rounded-full bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none text-[11px] font-semibold inline-flex items-center gap-1 cursor-pointer disabled:opacity-35 focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
            >
              <PaperPlaneOutline class="w-3 h-3 text-zinc-950 rotate-45 block shrink-0" />
              <span class="text-zinc-950">Send</span>
            </Button>
          </div>
        </div>

        <div class="pt-2 max-h-24 overflow-y-auto text-[13px] leading-relaxed select-text">
          {#if liveVoiceText}
            {#if voiceFinalText}
              <span class="text-zinc-100">{voiceFinalText}</span>
            {/if}
            {#if voicePartialText}
              <span class="text-zinc-400 italic">{voiceFinalText ? ' ' : ''}{voicePartialText}</span>
            {/if}
          {:else}
            <span class="text-zinc-500 text-xs italic">Listening for speech...</span>
          {/if}
        </div>
      </div>
    {/if}

    <div class="mb-2 flex flex-wrap items-center justify-between gap-1.5 px-0.5">
      <div class="flex items-center gap-1.5">
        <Button
          id="chat-jd-btn"
          color="alternative"
          size="xs"
          onclick={openJdModal}
          class="h-6 px-2.5 py-0 rounded-full text-[11px] font-medium leading-none inline-flex items-center gap-1.5 cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0 transition-colors {hasJd ? 'bg-zinc-900 dark:bg-zinc-900 text-white dark:text-white border-emerald-500/50 dark:border-emerald-500/50 hover:bg-zinc-800 dark:hover:bg-zinc-800 hover:border-emerald-400/60 dark:hover:border-emerald-400/60' : 'bg-zinc-900/90 dark:bg-zinc-900/90 text-zinc-400 dark:text-zinc-400 hover:text-white dark:hover:text-white border-zinc-800 dark:border-zinc-800 hover:bg-zinc-800 dark:hover:bg-zinc-800'}"
        >
          <BriefcaseOutline class="w-3 h-3 shrink-0 {hasJd ? 'text-emerald-400' : ''}" />
          <span class="leading-none">{hasJd ? 'JD Active' : '+ JD'}</span>
        </Button>
        <Tooltip triggeredBy="#chat-jd-btn" placement="top" class={TOOLTIP_CLASS}>
          {hasJd ? 'Edit active Job Description (Ctrl + J)' : 'Tailor answers with Job Description (Ctrl + J)'}
        </Tooltip>

        <Button
          id="chat-voice-source-btn"
          color="alternative"
          size="xs"
          onclick={handleSwitchVoiceSource}
          class="h-6 px-2.5 py-0 rounded-full text-[11px] font-medium leading-none inline-flex items-center gap-1.5 cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0 transition-colors {voiceSource === 'system' ? 'bg-zinc-900 dark:bg-zinc-900 text-white dark:text-white border-emerald-500/50 dark:border-emerald-500/50 hover:bg-zinc-800 dark:hover:bg-zinc-800' : 'bg-zinc-900/90 dark:bg-zinc-900/90 text-zinc-400 dark:text-zinc-400 hover:text-white dark:hover:text-white border-zinc-800 dark:border-zinc-800 hover:bg-zinc-800 dark:hover:bg-zinc-800'}"
        >
          {#if voiceSource === 'system'}
            <DesktopPcOutline class="w-3 h-3 shrink-0 text-emerald-400" />
            <span class="leading-none">System</span>
          {:else}
            <MicrophoneOutline class="w-3 h-3 shrink-0" />
            <span class="leading-none">Mic</span>
          {/if}
        </Button>
        <Tooltip triggeredBy="#chat-voice-source-btn" placement="top" class={TOOLTIP_CLASS}>
          {voiceSource === 'system'
            ? 'Capturing system audio (interviewer) — click for Microphone'
            : 'Capturing microphone — click for System Audio (interviewer)'}
        </Tooltip>
      </div>

      <div class="flex items-center gap-1">
        <Button
          id="chat-quick-answer-btn"
          color="alternative"
          size="xs"
          disabled={isLoading}
          onclick={answerScreen}
          class="h-6 px-2 py-0 rounded-full text-[10.5px] font-medium leading-none bg-zinc-900/90 dark:bg-zinc-900/90 text-zinc-300 dark:text-zinc-300 hover:text-white dark:hover:text-white border-zinc-800 dark:border-zinc-800 hover:bg-zinc-800 dark:hover:bg-zinc-800 cursor-pointer focus:ring-0 disabled:opacity-40"
        >
          Answer Screen
        </Button>
        <Tooltip triggeredBy="#chat-quick-answer-btn" placement="top" class={TOOLTIP_CLASS}>
          Answer current screen directly (Ctrl + Alt + Enter)
        </Tooltip>

        <Button
          color="alternative"
          size="xs"
          disabled={isLoading}
          onclick={() => handleSendMessage('Summarize what is currently on my screen in concise bullet points.', { forceScreenshot: true })}
          class="h-6 px-2 py-0 rounded-full text-[10.5px] font-medium leading-none bg-zinc-900/90 dark:bg-zinc-900/90 text-zinc-400 dark:text-zinc-400 hover:text-white dark:hover:text-white border-zinc-800 dark:border-zinc-800 hover:bg-zinc-800 dark:hover:bg-zinc-800 cursor-pointer focus:ring-0 disabled:opacity-40"
        >
          Summarize
        </Button>

        {#if messages.length > 0}
          <Button
            color="alternative"
            size="xs"
            disabled={isLoading}
            onclick={() => handleSendMessage('Make the previous answer much shorter and direct so I can say it in 20 seconds.')}
            class="h-6 px-2 py-0 rounded-full text-[10.5px] font-medium leading-none bg-zinc-900/90 dark:bg-zinc-900/90 text-zinc-400 dark:text-zinc-400 hover:text-white dark:hover:text-white border-zinc-800 dark:border-zinc-800 hover:bg-zinc-800 dark:hover:bg-zinc-800 cursor-pointer focus:ring-0 disabled:opacity-40"
          >
            Shorten
          </Button>
        {/if}
      </div>
    </div>

    <div class="flex items-center gap-2 bg-zinc-900/80 border border-zinc-800 rounded-2xl p-1.5 overflow-hidden focus-within:border-zinc-500 transition-colors">
      <Button
        id="chat-attach-screen-btn"
        color="alternative"
        size="xs"
        class="h-8 px-2.5 py-0 rounded-xl text-xs font-medium leading-none inline-flex items-center justify-center gap-1.5 shrink-0 cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0 transition-colors {attachScreenshot ? 'bg-zinc-800 dark:bg-zinc-800 text-white dark:text-white border-emerald-500/50 dark:border-emerald-500/50 hover:bg-zinc-700 dark:hover:bg-zinc-700 hover:border-emerald-400/60 dark:hover:border-emerald-400/60' : 'bg-transparent dark:bg-transparent text-zinc-500 dark:text-zinc-500 border-zinc-800 dark:border-zinc-800 hover:text-zinc-300 dark:hover:text-zinc-300 hover:bg-zinc-900 dark:hover:bg-zinc-900'}"
        onclick={() => (attachScreenshot = !attachScreenshot)}
      >
        <CameraPhotoOutline class="w-3.5 h-3.5 shrink-0 block {attachScreenshot ? 'text-emerald-400' : 'opacity-60'}" />
        <span class="leading-none {attachScreenshot ? '' : 'line-through opacity-75'}">Screen</span>
      </Button>
      <Tooltip triggeredBy="#chat-attach-screen-btn" placement="top" class={TOOLTIP_CLASS}>
        {attachScreenshot ? 'Screen capture included' : 'Text only (click to include screen)'}
      </Tooltip>

      <div class="flex-1 min-w-0 overflow-hidden flex items-center">
        <Textarea
          bind:value={inputText}
          bind:elementRef={textareaRef}
          onkeydown={handleKeydown}
          placeholder="Ask anything..."
          rows="1"
          classes={{ div: 'w-full min-w-0' }}
          class="block w-full min-h-0 bg-transparent dark:bg-transparent border-none dark:border-none text-[13px] leading-5 font-sans font-normal tracking-normal text-zinc-100 dark:text-zinc-100 placeholder:text-zinc-500 dark:placeholder:text-zinc-500 placeholder:font-normal outline-none resize-none max-h-28 overflow-y-auto py-1.5 px-1.5 rounded-none focus:ring-0 dark:focus:ring-0 shadow-none"
        />
      </div>

      <Button
        id="chat-voice-btn"
        color="alternative"
        size="xs"
        class="w-8 h-8 p-0 rounded-full transition-colors shrink-0 cursor-pointer flex items-center justify-center focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0 {isListening
          ? 'bg-emerald-500/15 dark:bg-emerald-500/15 border border-emerald-500/60 dark:border-emerald-500/60 text-emerald-400 dark:text-emerald-400 hover:bg-emerald-500/25 dark:hover:bg-emerald-500/25 hover:text-emerald-300 dark:hover:text-emerald-300 animate-pulse'
          : 'bg-zinc-800/80 dark:bg-zinc-800/80 border border-zinc-700/70 dark:border-zinc-700/70 text-zinc-300 dark:text-zinc-300 hover:bg-zinc-700 dark:hover:bg-zinc-700 hover:text-white dark:hover:text-white'}"
        onclick={handleToggleVoice}
        aria-label={isListening ? 'Stop voice input' : 'Start voice input'}
      >
        {#if isListening}
          <MicrophoneSolid class="w-4 h-4 text-emerald-400 block" />
        {:else}
          <MicrophoneOutline class="w-4 h-4 block" />
        {/if}
      </Button>
      <Tooltip triggeredBy="#chat-voice-btn" placement="top" class={TOOLTIP_CLASS}>
        {isListening ? 'Stop voice input (Ctrl + Alt + V)' : 'Live voice input (Ctrl + Alt + V)'}
      </Tooltip>

      <Button
        color="alternative"
        size="xs"
        disabled={!inputText.trim() || isLoading}
        class="w-8 h-8 p-0 rounded-full bg-white dark:bg-white text-zinc-950 dark:text-zinc-950 hover:bg-zinc-200 dark:hover:bg-zinc-200 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none dark:border-none transition-colors disabled:opacity-35 shrink-0 cursor-pointer shadow-sm flex items-center justify-center focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
        title="Send message"
        onclick={() => handleSendMessage()}
      >
        <PaperPlaneOutline class="w-3.5 h-3.5 text-zinc-950 rotate-45 translate-x-[1px] -translate-y-[1px] block" />
      </Button>
    </div>
  </div>
</div>

<!-- LLM API Key Required Modal -->
<Modal
  bind:open={llmSetupModalOpen}
  title="{activeLlmLabel} API Key Required"
  size="xs"
  class={MODAL_CLASS}
  classes={MODAL_CLASSES}
>
  <p class="text-xs text-zinc-300 leading-relaxed">
    AI Chat will not work until a <strong class="text-white">{activeLlmLabel} API key</strong> is added. Configure your key in Settings to start chatting.
  </p>

  {#snippet footer()}
    <div class="w-full flex items-center justify-end gap-2">
      <Button
        color="alternative"
        size="xs"
        onclick={() => (llmSetupModalOpen = false)}
        class="h-8 px-3 py-0 rounded-lg text-xs text-zinc-400 dark:text-zinc-400 hover:text-white border-zinc-800 dark:border-zinc-800 bg-transparent dark:bg-transparent cursor-pointer focus:ring-0"
      >
        Cancel
      </Button>
      <Button
        color="alternative"
        size="xs"
        onclick={() => {
          llmSetupModalOpen = false;
          onOpenSettings('model');
        }}
        class="h-8 px-3.5 py-0 rounded-lg bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none text-xs font-semibold cursor-pointer focus:ring-0"
      >
        <span class="text-zinc-950">Configure API Key</span>
      </Button>
    </div>
  {/snippet}
</Modal>

<!-- Job Description Modal (Chat-Window Scoped) -->
<Modal
  bind:open={jdModalOpen}
  title="Job Description"
  size="xs"
  class={MODAL_CLASS}
  classes={MODAL_CLASSES}
>
  <p class="text-[11.5px] text-zinc-400 leading-snug">
    Paste the role title, key skills, and expectations to tailor interview answers alongside your resume.
  </p>

  <div class="w-full rounded-xl border border-zinc-800 bg-zinc-900/80 overflow-hidden focus-within:border-white transition-colors p-2.5">
    <Textarea
      bind:value={jdDraft}
      bind:elementRef={jdTextareaRef}
      rows="6"
      maxlength="14000"
      placeholder="e.g. Senior Backend Engineer — Rust, Tokio, distributed systems..."
      classes={{ div: 'w-full' }}
      class="w-full max-h-36 overflow-y-auto bg-transparent dark:bg-transparent border-none dark:border-none text-zinc-100 dark:text-zinc-100 placeholder-zinc-500 dark:placeholder-zinc-500 text-xs font-sans p-0 pr-1 rounded-none focus:ring-0 dark:focus:ring-0 resize-none leading-relaxed shadow-none"
    />
  </div>

  <div class="flex items-center justify-between text-[10.5px] text-zinc-500 tabular-nums">
    <span>Applies directly in Chat</span>
    <span>{jdDraft.trim().length.toLocaleString()} / 14,000 chars</span>
  </div>

  {#if jdError}
    <p class="text-xs text-zinc-200 bg-zinc-900 border border-zinc-700 rounded-lg p-2.5">{jdError}</p>
  {/if}

  {#snippet footer()}
    <div class="w-full flex items-center justify-between gap-2">
      <div>
        {#if hasJd || jdDraft.trim()}
          <Button
            color="alternative"
            size="xs"
            disabled={savingJd}
            onclick={handleClearJd}
            class="h-8 px-3 py-0 rounded-lg text-xs text-zinc-400 dark:text-zinc-400 hover:text-white border-zinc-800 dark:border-zinc-800 bg-transparent dark:bg-transparent cursor-pointer focus:ring-0"
          >
            Clear
          </Button>
        {/if}
      </div>
      <div class="flex items-center gap-2">
        <Button
          color="alternative"
          size="xs"
          disabled={savingJd}
          onclick={() => (jdModalOpen = false)}
          class="h-8 px-3 py-0 rounded-lg text-xs text-zinc-400 dark:text-zinc-400 hover:text-white border-zinc-800 dark:border-zinc-800 bg-transparent dark:bg-transparent cursor-pointer focus:ring-0"
        >
          Cancel
        </Button>
        <Button
          color="alternative"
          size="xs"
          disabled={savingJd}
          onclick={handleSaveJd}
          class="h-8 px-3.5 py-0 rounded-lg bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none text-xs font-semibold cursor-pointer focus:ring-0"
        >
          <span class="text-zinc-950">{savingJd ? 'Saving...' : 'Save JD'}</span>
        </Button>
      </div>
    </div>
  {/snippet}
</Modal>

<!-- Voice Provider Setup Modal -->
<Modal
  bind:open={voiceSetupModalOpen}
  title={voiceSetupProvider === 'deepgram' ? 'Deepgram Voice Setup' : 'Offline Voice Input (Vosk)'}
  size="xs"
  class={MODAL_CLASS}
  classes={MODAL_CLASSES}
>
  {#if voiceSetupProvider === 'deepgram'}
    <p class="text-xs text-zinc-300 leading-relaxed">
      Voice Input will not work until a <strong class="text-white">Deepgram API key</strong> is configured (or switch to Vosk Local in Settings).
    </p>
  {:else}
    <p class="text-xs text-zinc-300 leading-relaxed">
      Voice Input will not work until Vosk (~58 MB) is downloaded or you configure <strong class="text-white">Deepgram Cloud</strong> in Settings.
    </p>

    {#if installingVoice}
      <div class="space-y-1.5">
        <div class="w-full h-1.5 bg-zinc-800 rounded-full overflow-hidden">
          <div class="h-full bg-white transition-all duration-200" style="width: {voiceSetupProgress.percent}%"></div>
        </div>
        <div class="flex items-center justify-between text-[11px] text-zinc-400 tabular-nums">
          <span class="truncate">{voiceSetupProgress.message || 'Downloading...'}</span>
          <span>{voiceSetupProgress.percent}%</span>
        </div>
      </div>
    {/if}
  {/if}

  {#if voiceSetupError}
    <p class="text-xs text-zinc-300 bg-zinc-900 border border-zinc-700 rounded-lg p-2.5">{voiceSetupError}</p>
  {/if}

  {#snippet footer()}
    <div class="w-full flex items-center justify-end gap-2">
      <Button
        color="alternative"
        size="xs"
        disabled={installingVoice}
        onclick={() => (voiceSetupModalOpen = false)}
        class="h-8 px-3 py-0 rounded-lg text-xs text-zinc-400 dark:text-zinc-400 hover:text-white border-zinc-800 dark:border-zinc-800 bg-transparent dark:bg-transparent cursor-pointer focus:ring-0"
      >
        Cancel
      </Button>
      <Button
        color="alternative"
        size="xs"
        disabled={installingVoice}
        onclick={() => {
          voiceSetupModalOpen = false;
          onOpenSettings('voice');
        }}
        class="h-8 px-3 py-0 rounded-lg text-xs font-medium cursor-pointer focus:ring-0 {voiceSetupProvider === 'deepgram' ? 'bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none font-semibold' : 'bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 dark:hover:bg-zinc-800 text-zinc-200 dark:text-zinc-200 hover:text-white border-zinc-700 dark:border-zinc-700'}"
      >
        <span class={voiceSetupProvider === 'deepgram' ? 'text-zinc-950' : ''}>Voice Settings</span>
      </Button>
      {#if voiceSetupProvider !== 'deepgram'}
        <Button
          color="alternative"
          size="xs"
          disabled={installingVoice}
          onclick={handleInstallAndStartVoice}
          class="h-8 px-3.5 py-0 rounded-lg bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none text-xs font-semibold cursor-pointer focus:ring-0"
        >
          <span class="text-zinc-950">{installingVoice ? 'Downloading...' : 'Download & Enable'}</span>
        </Button>
      {/if}
    </div>
  {/snippet}
</Modal>
