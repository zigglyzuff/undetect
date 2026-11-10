<script>
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import {
    Badge,
    Button,
    Card,
    Dropdown,
    DropdownGroup,
    DropdownItem,
    Input,
    Label,
    Toggle
  } from 'flowbite-svelte';
  import {
    CheckOutline,
    ChevronDownOutline,
    ChevronRightOutline,
    MicrophoneOutline,
    TrashBinOutline
  } from 'flowbite-svelte-icons';
  import {
    DROPDOWN_CLASS,
    DROPDOWN_GROUP_CLASS,
    getDropdownItemClass,
    syncDropdownWidth
  } from '../lib/dropdownClasses.js';

  export let showToast = (msg) => {};

  const VOICE_PROVIDER_ITEMS = [
    { value: 'deepgram', name: 'Deepgram Cloud', tag: 'Recommended • Live API' },
    { value: 'vosk', name: 'Vosk Local', tag: 'Offline • On-Device' },
  ];

  const DEEPGRAM_MODEL_ITEMS = [
    { value: 'nova-3', name: 'nova-3', tag: 'Recommended' },
    { value: 'nova-2', name: 'nova-2', tag: 'Fast' },
    { value: 'nova-3-general', name: 'nova-3-general', tag: 'General' },
    { value: 'nova-2-meeting', name: 'nova-2-meeting', tag: 'Meetings' },
  ];

  const DEEPGRAM_LANGUAGE_ITEMS = [
    { value: 'en-US', name: 'English (US)', tag: 'en-US' },
    { value: 'en', name: 'English (Global)', tag: 'en' },
    { value: 'multi', name: 'Multilingual', tag: 'multi' },
  ];

  const SOURCE_OPTIONS = [
    { value: 'system', name: 'System Audio (Loopback)' },
    { value: 'mic', name: 'Microphone' },
  ];

  let voiceStatus = {
    provider: 'deepgram',
    runtime_installed: false,
    model_installed: false,
    has_deepgram_key: false,
    deepgram_key_masked: '',
    deepgram_model: 'nova-3',
    deepgram_language: 'en-US',
    auto_send: false,
    ready: false,
    listening: false,
    installing: false,
    source: 'system',
    device_name: null,
    custom_runtime_dir: '',
    custom_model_dir: '',
    error: null,
  };

  let devices = [];
  let provider = 'deepgram';
  let deepgramApiKey = '';
  let deepgramModel = 'nova-3';
  let deepgramLanguage = 'en-US';
  let autoSend = false;
  let source = 'system';
  let deviceName = '';
  let customRuntimeDir = '';
  let customModelDir = '';

  const monoToggleClasses = {
    span: 'me-0 border-0 ring-1 ring-inset ring-zinc-700 bg-zinc-800 dark:bg-zinc-800 after:border-0 after:top-[2px] after:start-[2px] after:h-4 after:w-4 after:bg-zinc-300 peer-checked:bg-white dark:peer-checked:bg-white peer-checked:ring-white peer-checked:after:bg-zinc-950 peer-focus:ring-0 dark:peer-focus:ring-0',
  };

  let showCustomPaths = false;
  let clearingKey = false;
  let providerDropdownOpen = false;
  let dgModelDropdownOpen = false;
  let dgLangDropdownOpen = false;
  let sourceDropdownOpen = false;
  let deviceDropdownOpen = false;
  let installing = false;
  let setupProgress = { percent: 0, message: '' };
  let unlistenFns = [];

  $: selectedProviderObj =
    VOICE_PROVIDER_ITEMS.find((p) => p.value === provider) || VOICE_PROVIDER_ITEMS[0];
  $: selectedDgModelObj =
    DEEPGRAM_MODEL_ITEMS.find((m) => m.value === deepgramModel) || {
      value: deepgramModel,
      name: deepgramModel || 'nova-3',
      tag: '',
    };
  $: selectedDgLangObj =
    DEEPGRAM_LANGUAGE_ITEMS.find((l) => l.value === deepgramLanguage) || {
      value: deepgramLanguage,
      name: deepgramLanguage || 'en-US',
      tag: '',
    };
  $: selectedSourceObj = SOURCE_OPTIONS.find((s) => s.value === source) || SOURCE_OPTIONS[0];
  $: filteredDevices = devices.filter((d) => d.kind === source);

  export async function refreshVoice() {
    try {
      const [statusRes, devRes] = await Promise.all([
        invoke('get_voice_status'),
        invoke('list_voice_devices').catch(() => []),
      ]);
      if (statusRes) {
        voiceStatus = statusRes;
        provider = statusRes.provider || 'deepgram';
        deepgramModel = statusRes.deepgram_model || 'nova-3';
        deepgramLanguage = statusRes.deepgram_language || 'en-US';
        autoSend = Boolean(statusRes.auto_send);
        source = statusRes.source || 'system';
        deviceName = statusRes.device_name || '';
        customRuntimeDir = statusRes.custom_runtime_dir || '';
        customModelDir = statusRes.custom_model_dir || '';
        installing = Boolean(statusRes.installing);
      }
      if (Array.isArray(devRes)) {
        devices = devRes;
      }
    } catch (e) {
      console.warn('Failed to fetch voice status:', e);
    }
  }

  export async function saveVoiceConfig() {
    try {
      const updated = await invoke('set_voice_config', {
        config: {
          provider,
          source,
          device_name: deviceName || null,
          custom_runtime_dir: customRuntimeDir || null,
          custom_model_dir: customModelDir || null,
          deepgram_api_key: deepgramApiKey,
          deepgram_model: deepgramModel || 'nova-3',
          deepgram_language: deepgramLanguage || 'en-US',
          auto_send: autoSend,
        },
      });
      if (updated) {
        voiceStatus = updated;
        if (updated.has_deepgram_key) {
          deepgramApiKey = '';
        }
        window.dispatchEvent(new CustomEvent('undetect:config-updated'));
      }
    } catch (e) {
      showToast('Voice config error: ' + e);
    }
  }

  async function handleClearDeepgramKey() {
    if (clearingKey) return;
    clearingKey = true;
    try {
      const updated = await invoke('clear_voice_api_key');
      if (updated) {
        voiceStatus = updated;
      }
      deepgramApiKey = '';
      window.dispatchEvent(new CustomEvent('undetect:config-updated'));
      showToast('Deepgram API key cleared.');
    } catch (e) {
      showToast('Failed to clear Deepgram key: ' + e);
    } finally {
      clearingKey = false;
    }
  }

  async function handleSelectProvider(val) {
    provider = val;
    providerDropdownOpen = false;
    await saveVoiceConfig();
  }

  async function handleSelectDgModel(val) {
    deepgramModel = val;
    dgModelDropdownOpen = false;
    await saveVoiceConfig();
  }

  async function handleSelectDgLang(val) {
    deepgramLanguage = val;
    dgLangDropdownOpen = false;
    await saveVoiceConfig();
  }

  async function handleSelectSource(val) {
    source = val;
    deviceName = '';
    sourceDropdownOpen = false;
    await saveVoiceConfig();
  }

  async function handleSelectDevice(name) {
    deviceName = name;
    deviceDropdownOpen = false;
    await saveVoiceConfig();
  }

  async function handleSetupVosk() {
    if (installing) return;
    installing = true;
    setupProgress = { percent: 4, message: 'Preparing Vosk offline engine...' };
    try {
      const res = await invoke('setup_vosk');
      if (res) voiceStatus = res;
      showToast('Offline voice ready!');
    } catch (e) {
      showToast('Voice setup failed: ' + e);
    } finally {
      installing = false;
      await refreshVoice();
    }
  }

  onMount(async () => {
    await refreshVoice();
    try {
      unlistenFns = await Promise.all([
        listen('vosk://setup-progress', ({ payload }) => {
          if (payload) setupProgress = payload;
        }),
        listen('vosk://state', ({ payload }) => {
          voiceStatus = { ...voiceStatus, listening: Boolean(payload) };
        }),
      ]);
    } catch (e) {}
  });

  onDestroy(() => {
    for (const u of unlistenFns) {
      if (typeof u === 'function') u();
    }
  });
</script>

<Card
  size="xl"
  color="gray"
  class="w-full bg-zinc-900/45 dark:bg-zinc-900/45 border border-zinc-800/90 dark:border-zinc-800/90 rounded-2xl p-4 flex flex-col gap-3 shadow-sm"
>
  <div class="flex items-center justify-between pb-2.5 border-b border-zinc-800/80">
    <div class="flex items-center gap-2 text-[13px] font-semibold text-zinc-100">
      <MicrophoneOutline class="w-4 h-4 text-zinc-400 shrink-0" />
      <span>Voice Input</span>
    </div>
    <Badge color="gray" class="bg-zinc-800 dark:bg-zinc-800 border border-zinc-700/70 dark:border-zinc-700/70 text-zinc-200 dark:text-zinc-200 rounded-full text-[11px] font-sans font-medium px-2.5 py-0.5 leading-none inline-flex items-center gap-1.5">
      <span class="w-1.5 h-1.5 rounded-full {voiceStatus.listening ? 'bg-emerald-400 animate-pulse' : (provider === 'deepgram' ? voiceStatus.has_deepgram_key : voiceStatus.ready) ? 'bg-emerald-400' : 'bg-zinc-500'}"></span>
      <span>
        {voiceStatus.listening
          ? 'Listening'
          : provider === 'deepgram'
            ? (voiceStatus.has_deepgram_key ? 'Deepgram Ready' : 'Key Required')
            : (voiceStatus.runtime_installed && voiceStatus.model_installed ? 'Vosk Ready' : 'Setup Needed')}
      </span>
    </Badge>
  </div>

  <!-- Speech Provider Selector -->
  <div class="flex flex-col gap-1.5">
    <Label for="voice-provider-btn" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">Speech Provider</Label>
    <Button
      id="voice-provider-btn"
      color="alternative"
      size="sm"
      class="w-full justify-between px-3 py-2 bg-zinc-950 dark:bg-zinc-950 hover:bg-zinc-900 dark:hover:bg-zinc-900 border border-zinc-800 dark:border-zinc-800 hover:border-zinc-600 text-zinc-100 dark:text-zinc-100 text-xs font-sans font-medium rounded-xl focus:ring-0 cursor-pointer"
    >
      <span class="pointer-events-none flex items-center gap-2 min-w-0 truncate">
        <span class="font-semibold text-zinc-100">{selectedProviderObj.name}</span>
        <Badge color="gray" class="bg-zinc-800 dark:bg-zinc-800 text-zinc-300 dark:text-zinc-300 border border-zinc-700 dark:border-zinc-700 rounded-full text-[10px] font-sans font-medium px-2 py-0.5 leading-none">
          {selectedProviderObj.tag}
        </Badge>
      </span>
      <ChevronDownOutline class="w-3.5 h-3.5 text-zinc-400 shrink-0 pointer-events-none" />
    </Button>

    <Dropdown
      bind:isOpen={providerDropdownOpen}
      triggeredBy="#voice-provider-btn"
      placement="bottom-start"
      strategy="fixed"
      triggerDelay={0}
      offset={4}
      onbeforetoggle={syncDropdownWidth}
      class={DROPDOWN_CLASS}
    >
      <DropdownGroup class={DROPDOWN_GROUP_CLASS}>
        {#each VOICE_PROVIDER_ITEMS as item (item.value)}
          <DropdownItem
            onclick={() => handleSelectProvider(item.value)}
            class={getDropdownItemClass(provider === item.value)}
          >
            <div class="flex items-center gap-2 min-w-0">
              <span class="truncate">{item.name}</span>
              <Badge color="gray" class="bg-zinc-900 dark:bg-zinc-900 border border-zinc-700 dark:border-zinc-700 text-zinc-400 dark:text-zinc-400 rounded-full text-[10px] font-sans font-medium px-2 py-0.5 leading-none">
                {item.tag}
              </Badge>
            </div>
            {#if provider === item.value}
              <CheckOutline class="w-3.5 h-3.5 text-white shrink-0" />
            {/if}
          </DropdownItem>
        {/each}
      </DropdownGroup>
    </Dropdown>
  </div>

  <!-- Selected Speech Provider Configuration (Clean Single-Provider Panel) -->
  {#if provider === 'deepgram'}
    <div class="p-3.5 rounded-xl bg-zinc-950/75 border border-zinc-800/80 flex flex-col gap-3">
      <div class="flex items-center justify-between">
        <span class="text-xs font-semibold text-zinc-200">Deepgram Streaming Setup</span>
        <a
          href="https://console.deepgram.com/"
          target="_blank"
          class="text-[11px] font-sans font-medium text-zinc-400 hover:text-white underline underline-offset-2"
        >
          Get API Key ↗
        </a>
      </div>

      <div class="flex flex-col gap-1.5">
        <div class="flex items-center justify-between gap-2">
          <Label for="deepgram-api-key" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">Deepgram API Key</Label>
          {#if voiceStatus.has_deepgram_key}
            <button
              type="button"
              disabled={clearingKey}
              onclick={handleClearDeepgramKey}
              class="inline-flex items-center gap-1 text-[11px] font-medium text-zinc-400 hover:text-white disabled:opacity-50 cursor-pointer transition-colors"
              title="Clear saved Deepgram API key"
            >
              <TrashBinOutline class="w-3 h-3" />
              <span>{clearingKey ? 'Clearing...' : 'Clear saved key'}</span>
            </button>
          {/if}
        </div>
        <Input
          id="deepgram-api-key"
          type="password"
          bind:value={deepgramApiKey}
          placeholder={voiceStatus.has_deepgram_key ? `Saved (${voiceStatus.deepgram_key_masked})` : 'Paste Deepgram API key...'}
          size="sm"
          class="w-full bg-zinc-950 dark:bg-zinc-950 border-zinc-800 dark:border-zinc-800 text-zinc-100 dark:text-zinc-100 placeholder-zinc-500 dark:placeholder-zinc-500 text-xs font-sans rounded-xl focus:border-white dark:focus:border-white focus:ring-0"
        />
        {#if !voiceStatus.has_deepgram_key && !deepgramApiKey.trim()}
          <p class="text-[11px] text-zinc-400 leading-snug">
            No Deepgram key saved. Cloud voice input won't work until you add a key and save.
          </p>
        {/if}
      </div>

      <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div class="flex flex-col gap-1.5">
          <Label for="dg-model-btn" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">Model</Label>
          <Button
            id="dg-model-btn"
            color="alternative"
            size="sm"
            class="w-full justify-between px-3 py-2 bg-zinc-950 dark:bg-zinc-950 hover:bg-zinc-900 dark:hover:bg-zinc-900 border border-zinc-800 dark:border-zinc-800 hover:border-zinc-600 text-zinc-100 dark:text-zinc-100 text-xs font-sans font-medium rounded-xl focus:ring-0 cursor-pointer"
          >
            <span class="truncate">{selectedDgModelObj.name}</span>
            <ChevronDownOutline class="w-3.5 h-3.5 text-zinc-400 shrink-0 pointer-events-none" />
          </Button>

          <Dropdown
            bind:isOpen={dgModelDropdownOpen}
            triggeredBy="#dg-model-btn"
            placement="bottom-start"
            strategy="fixed"
            triggerDelay={0}
            offset={4}
            onbeforetoggle={syncDropdownWidth}
            class={DROPDOWN_CLASS}
          >
            <DropdownGroup class={DROPDOWN_GROUP_CLASS}>
              {#each DEEPGRAM_MODEL_ITEMS as item (item.value)}
                <DropdownItem
                  onclick={() => handleSelectDgModel(item.value)}
                  class={getDropdownItemClass(deepgramModel === item.value)}
                >
                  <div class="flex items-center gap-2 min-w-0">
                    <span class="truncate">{item.name}</span>
                    {#if item.tag}
                      <Badge color="gray" class="bg-zinc-900 dark:bg-zinc-900 border border-zinc-700 dark:border-zinc-700 text-zinc-400 dark:text-zinc-400 rounded-full text-[10px] font-sans font-medium px-2 py-0.5 leading-none">
                        {item.tag}
                      </Badge>
                    {/if}
                  </div>
                  {#if deepgramModel === item.value}
                    <CheckOutline class="w-3.5 h-3.5 text-white shrink-0" />
                  {/if}
                </DropdownItem>
              {/each}
            </DropdownGroup>
          </Dropdown>
        </div>

        <div class="flex flex-col gap-1.5">
          <Label for="dg-lang-btn" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">Language</Label>
          <Button
            id="dg-lang-btn"
            color="alternative"
            size="sm"
            class="w-full justify-between px-3 py-2 bg-zinc-950 dark:bg-zinc-950 hover:bg-zinc-900 dark:hover:bg-zinc-900 border border-zinc-800 dark:border-zinc-800 hover:border-zinc-600 text-zinc-100 dark:text-zinc-100 text-xs font-sans font-medium rounded-xl focus:ring-0 cursor-pointer"
          >
            <span class="truncate">{selectedDgLangObj.name}</span>
            <ChevronDownOutline class="w-3.5 h-3.5 text-zinc-400 shrink-0 pointer-events-none" />
          </Button>

          <Dropdown
            bind:isOpen={dgLangDropdownOpen}
            triggeredBy="#dg-lang-btn"
            placement="bottom-start"
            strategy="fixed"
            triggerDelay={0}
            offset={4}
            onbeforetoggle={syncDropdownWidth}
            class={DROPDOWN_CLASS}
          >
            <DropdownGroup class={DROPDOWN_GROUP_CLASS}>
              {#each DEEPGRAM_LANGUAGE_ITEMS as item (item.value)}
                <DropdownItem
                  onclick={() => handleSelectDgLang(item.value)}
                  class={getDropdownItemClass(deepgramLanguage === item.value)}
                >
                  <span class="truncate">{item.name}</span>
                  {#if deepgramLanguage === item.value}
                    <CheckOutline class="w-3.5 h-3.5 text-white shrink-0" />
                  {/if}
                </DropdownItem>
              {/each}
            </DropdownGroup>
          </Dropdown>
        </div>
      </div>
    </div>
  {:else}
    <div class="p-3.5 rounded-xl bg-zinc-950/75 border border-zinc-800/80 flex flex-col gap-2.5">
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <div class="text-[12.5px] font-medium text-zinc-100">
            {voiceStatus.runtime_installed && voiceStatus.model_installed ? 'Offline Speech Engine Installed' : 'Setup Offline Speech Engine'}
          </div>
          <div class="text-[11px] text-zinc-400 leading-snug mt-0.5">
            {voiceStatus.runtime_installed && voiceStatus.model_installed
              ? 'Runs 100% locally via Vosk.'
              : 'Downloads Vosk runtime & English model (~58 MB once).'}
          </div>
        </div>

        {#if !(voiceStatus.runtime_installed && voiceStatus.model_installed)}
          <Button
            color="alternative"
            size="xs"
            disabled={installing}
            onclick={handleSetupVosk}
            class="h-8 px-3 py-0 rounded-xl bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none text-xs font-sans font-semibold shrink-0 cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
          >
            <span class="text-zinc-950">{installing ? 'Installing...' : 'Download'}</span>
          </Button>
        {/if}
      </div>

      {#if installing}
        <div class="flex flex-col gap-1.5">
          <div class="w-full h-1.5 bg-zinc-800 rounded-full overflow-hidden">
            <div class="h-full bg-white transition-all duration-200" style="width: {setupProgress.percent}%"></div>
          </div>
          <div class="flex items-center justify-between text-[10.5px] text-zinc-400 tabular-nums">
            <span class="truncate">{setupProgress.message || 'Downloading...'}</span>
            <span>{setupProgress.percent}%</span>
          </div>
        </div>
      {/if}

      <Button
        color="alternative"
        size="xs"
        aria-expanded={showCustomPaths}
        onclick={() => (showCustomPaths = !showCustomPaths)}
        class="w-full justify-between px-3 py-1.5 rounded-lg bg-zinc-900/70 dark:bg-zinc-900/70 hover:bg-zinc-800 dark:hover:bg-zinc-800 border-zinc-800 dark:border-zinc-800 text-zinc-400 dark:text-zinc-400 hover:text-zinc-200 text-[11px] font-sans font-medium cursor-pointer focus:ring-0"
      >
        <span>Custom local model paths</span>
        {#if showCustomPaths}
          <ChevronDownOutline class="w-3 h-3 text-zinc-400" />
        {:else}
          <ChevronRightOutline class="w-3 h-3 text-zinc-400" />
        {/if}
      </Button>

      {#if showCustomPaths}
        <div class="flex flex-col gap-2.5 pt-1">
          <div class="flex flex-col gap-1">
            <Label for="vosk-model-dir" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">Custom Model Directory</Label>
            <Input
              id="vosk-model-dir"
              type="text"
              bind:value={customModelDir}
              placeholder="e.g. C:\vosk\vosk-model-en-us-0.22"
              size="sm"
              class="bg-zinc-950 dark:bg-zinc-950 border-zinc-800 dark:border-zinc-800 text-zinc-100 dark:text-zinc-100 placeholder-zinc-500 dark:placeholder-zinc-500 text-xs font-sans rounded-xl focus:border-white dark:focus:border-white focus:ring-0"
            />
          </div>
          <div class="flex flex-col gap-1">
            <Label for="vosk-runtime-dir" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">Custom Runtime Folder</Label>
            <Input
              id="vosk-runtime-dir"
              type="text"
              bind:value={customRuntimeDir}
              placeholder="Folder containing libvosk.dll"
              size="sm"
              class="bg-zinc-950 dark:bg-zinc-950 border-zinc-800 dark:border-zinc-800 text-zinc-100 dark:text-zinc-100 placeholder-zinc-500 dark:placeholder-zinc-500 text-xs font-sans rounded-xl focus:border-white dark:focus:border-white focus:ring-0"
            />
          </div>
        </div>
      {/if}
    </div>
  {/if}

  <!-- Shared Audio Capture Source & Device Selectors -->
  <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
    <div class="flex flex-col gap-1.5">
      <Label for="voice-source-btn" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">Audio Source</Label>
      <Button
        id="voice-source-btn"
        color="alternative"
        size="sm"
        class="w-full justify-between px-3 py-2 bg-zinc-950 dark:bg-zinc-950 hover:bg-zinc-900 dark:hover:bg-zinc-900 border border-zinc-800 dark:border-zinc-800 hover:border-zinc-600 text-zinc-100 dark:text-zinc-100 text-xs font-sans font-medium rounded-xl focus:ring-0 cursor-pointer"
      >
        <span class="truncate">{selectedSourceObj.name}</span>
        <ChevronDownOutline class="w-3.5 h-3.5 text-zinc-400 shrink-0 pointer-events-none" />
      </Button>

      <Dropdown
        bind:isOpen={sourceDropdownOpen}
        triggeredBy="#voice-source-btn"
        placement="bottom-start"
        strategy="fixed"
        triggerDelay={0}
        offset={4}
        onbeforetoggle={syncDropdownWidth}
        class={DROPDOWN_CLASS}
      >
        <DropdownGroup class={DROPDOWN_GROUP_CLASS}>
          {#each SOURCE_OPTIONS as opt (opt.value)}
            <DropdownItem
              onclick={() => handleSelectSource(opt.value)}
              class={getDropdownItemClass(source === opt.value)}
            >
              <span class="truncate">{opt.name}</span>
              {#if source === opt.value}
                <CheckOutline class="w-3.5 h-3.5 text-white shrink-0" />
              {/if}
            </DropdownItem>
          {/each}
        </DropdownGroup>
      </Dropdown>
    </div>

    <div class="flex flex-col gap-1.5">
      <Label for="voice-device-btn" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">Input Device</Label>
      <Button
        id="voice-device-btn"
        color="alternative"
        size="sm"
        class="w-full justify-between px-3 py-2 bg-zinc-950 dark:bg-zinc-950 hover:bg-zinc-900 dark:hover:bg-zinc-900 border border-zinc-800 dark:border-zinc-800 hover:border-zinc-600 text-zinc-100 dark:text-zinc-100 text-xs font-sans font-medium rounded-xl focus:ring-0 cursor-pointer"
      >
        <span class="truncate">{deviceName || 'System Default'}</span>
        <ChevronDownOutline class="w-3.5 h-3.5 text-zinc-400 shrink-0 pointer-events-none" />
      </Button>

      <Dropdown
        bind:isOpen={deviceDropdownOpen}
        triggeredBy="#voice-device-btn"
        placement="bottom-start"
        strategy="fixed"
        triggerDelay={0}
        offset={4}
        onbeforetoggle={syncDropdownWidth}
        class={DROPDOWN_CLASS}
      >
        <DropdownGroup class={DROPDOWN_GROUP_CLASS}>
          <DropdownItem
            onclick={() => handleSelectDevice('')}
            class={getDropdownItemClass(!deviceName)}
          >
            <span>System Default</span>
            {#if !deviceName}
              <CheckOutline class="w-3.5 h-3.5 text-white shrink-0" />
            {/if}
          </DropdownItem>
          {#each filteredDevices as dev (dev.name)}
            <DropdownItem
              onclick={() => handleSelectDevice(dev.name)}
              class={getDropdownItemClass(deviceName === dev.name)}
            >
              <span class="truncate">{dev.name}</span>
              {#if deviceName === dev.name}
                <CheckOutline class="w-3.5 h-3.5 text-white shrink-0" />
              {/if}
            </DropdownItem>
          {/each}
        </DropdownGroup>
      </Dropdown>
    </div>
  </div>

  <div class="w-full flex items-center justify-between gap-3 p-3 bg-zinc-950/75 border border-zinc-800/80 rounded-xl">
    <div class="min-w-0">
      <div class="text-[12px] font-medium text-zinc-100">Auto-Send Voice on Pause</div>
      <div class="text-[10.5px] text-zinc-400 leading-snug mt-0.5">Automatically send transcript to chat when you stop speaking.</div>
    </div>
    <Toggle
      color="gray"
      size="small"
      class="relative cursor-pointer"
      bind:checked={autoSend}
      onchange={saveVoiceConfig}
      classes={monoToggleClasses}
      aria-label="Auto-send voice transcript on pause"
    />
  </div>
</Card>
