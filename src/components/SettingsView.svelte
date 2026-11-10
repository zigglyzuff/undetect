<script>
  import { onMount } from 'svelte';
  import {
    Alert,
    Badge,
    Button,
    Card,
    Input,
    Label,
    Range,
    Toggle
  } from 'flowbite-svelte';
  import {
    ShieldCheckOutline,
    DesktopPcOutline,
    AdjustmentsVerticalOutline,
    CheckOutline,
    ArrowLeftOutline,
    RefreshOutline,
    KeyboardOutline,
    ChevronDownOutline,
    ChevronRightOutline
  } from 'flowbite-svelte-icons';
  import ModelProviderFields from './ModelProviderFields.svelte';
  import VoiceSettingsCard from './VoiceSettingsCard.svelte';
  import {
    ChatService,
    DEFAULT_GEMINI_MODEL,
    DEFAULT_GROQ_MODEL,
    formatGroqModelItems,
    normalizeGeminiModel
  } from '../chat/services/ChatService.js';
  import {
    GLOBAL_SHORTCUTS,
    IN_APP_SHORTCUTS,
    MORE_SHORTCUTS,
    WINDOW_SIZE_PRESETS
  } from '../lib/shortcuts.js';

  export let status = {
    running: false,
    port: 9090,
    password: '1234',
    content_protected: true,
    skip_taskbar: false,
    always_on_top: true,
    click_through: false,
    window_opacity: 100,
    telemetry_enabled: true,
    app_version: '2.1.0',
  };

  export let updateBanner = {
    show: false,
    version: '',
    downloadUrl: '',
  };
  export let isUpdating = false;

  export let windowSize = { width: 460, height: 710 };
  export let stealthCursor = true;
  export let activeSection = 'model';
  export let onClose = () => {};
  export let onSaveServer = (port, password) => {};
  export let onToggleProtection = (val) => {};
  export let onToggleTaskbar = (val) => {};
  export let onToggleAlwaysOnTop = (val) => {};
  export let onToggleClickThrough = (val) => {};
  export let onToggleStealthCursor = (val) => {};
  export let onToggleTelemetry = (val) => {};
  export let onSetWindowOpacity = (val) => {};
  export let onSetWindowSize = (w, h) => {};
  export let onStepResize = (dw, dh) => {};
  export let onResetWindowSize = () => {};
  export let onCheckUpdates = () => {};
  export let onDownloadAndInstallUpdate = () => {};
  export let showToast = (msg) => {};

  const SECTIONS = [
    { id: 'model', label: 'AI Model' },
    { id: 'voice', label: 'Voice' },
    { id: 'window', label: 'Window' },
    { id: 'server', label: 'Server' },
    { id: 'shortcuts', label: 'Shortcuts' },
  ];

  let settingsScrollContainer;
  let voiceSectionRef;
  let showMoreShortcuts = false;

  export function scrollContent(deltaY) {
    if (settingsScrollContainer) {
      settingsScrollContainer.scrollTop += deltaY;
    }
  }

  export function focusShortcutsSection() {
    activeSection = 'shortcuts';
    if (settingsScrollContainer) {
      settingsScrollContainer.scrollTop = 0;
    }
  }

  function selectSection(id) {
    activeSection = id;
    if (settingsScrollContainer) {
      settingsScrollContainer.scrollTop = 0;
    }
  }

  const chatService = new ChatService();

  let serverPort = status.port;
  let serverPassword = status.password;
  let windowOpacity = status.window_opacity ?? 100;
  let alwaysOnTop = status.always_on_top ?? true;
  let contentProtected = status.content_protected ?? true;
  let skipTaskbar = status.skip_taskbar ?? false;
  let clickThrough = status.click_through ?? false;
  let telemetryEnabled = status.telemetry_enabled ?? true;

  let activeProvider = 'groq';
  let geminiApiKey = '';
  let geminiModel = DEFAULT_GEMINI_MODEL;
  let groqApiKey = '';
  let groqModel = DEFAULT_GROQ_MODEL;
  let autoOcr = true;
  let systemPrompt = '';

  let groqModelItems = [];
  let loadingGroqModels = false;

  let config = {
    has_gemini_key: false,
    gemini_key_masked: '',
    has_groq_key: false,
    groq_key_masked: '',
    has_resume: false,
    resume_filename: '',
    resume_pages: 0,
    resume_chars: 0,
    resume_preview: '',
  };

  let saving = false;
  let saveFeedback = '';

  $: serverPort = status.port;
  $: serverPassword = status.password;
  $: alwaysOnTop = Boolean(status.always_on_top);
  $: contentProtected = Boolean(status.content_protected);
  $: skipTaskbar = Boolean(status.skip_taskbar);
  $: clickThrough = Boolean(status.click_through);
  $: telemetryEnabled = status.telemetry_enabled ?? true;
  $: if (typeof status.window_opacity === 'number') {
    windowOpacity = status.window_opacity;
  }
  $: modelKeyReady = activeProvider === 'groq' ? Boolean(config.has_groq_key) : Boolean(config.has_gemini_key);

  const monoInputClass =
    'bg-zinc-950 dark:bg-zinc-950 border-zinc-800 dark:border-zinc-800 text-zinc-100 dark:text-zinc-100 placeholder-zinc-500 dark:placeholder-zinc-500 text-xs font-sans tabular-nums rounded-xl focus:border-white dark:focus:border-white focus:ring-0 dark:focus:ring-0';

  const monoToggleClasses = {
    span: 'me-0 border-0 ring-1 ring-inset ring-zinc-700 bg-zinc-800 dark:bg-zinc-800 after:border-0 after:top-[2px] after:start-[2px] after:h-4 after:w-4 after:bg-zinc-300 peer-checked:bg-white dark:peer-checked:bg-white peer-checked:ring-white peer-checked:after:bg-zinc-950 peer-focus:ring-0 dark:peer-focus:ring-0',
  };

  onMount(async () => {
    await loadLlmConfig();
    await fetchGroqModels();
  });

  async function loadLlmConfig() {
    try {
      const cfg = await chatService.getConfig();
      if (cfg) {
        config = cfg;
        activeProvider = cfg.active_provider || 'groq';
        geminiModel = normalizeGeminiModel(cfg.gemini_model);
        groqModel = cfg.groq_model || DEFAULT_GROQ_MODEL;
        autoOcr = cfg.auto_ocr ?? true;
        systemPrompt = cfg.system_prompt || '';
      }
    } catch (e) {
      console.warn('Failed to load LLM config in settings:', e);
    }
  }

  async function fetchGroqModels() {
    loadingGroqModels = true;
    try {
      const models = await chatService.getGroqModels(groqApiKey || null);
      groqModelItems = formatGroqModelItems(models);

      if (!models.includes(groqModel)) {
        groqModel = models[0] || DEFAULT_GROQ_MODEL;
      }
    } catch (e) {
      console.warn('Failed to load Groq models in settings:', e);
    } finally {
      loadingGroqModels = false;
    }
  }

  async function handleSaveAll() {
    saving = true;
    saveFeedback = '';
    try {
      onSaveServer(Number(serverPort) || 9090, serverPassword);

      const payload = {
        active_provider: activeProvider,
        gemini_api_key: geminiApiKey,
        gemini_model: normalizeGeminiModel(geminiModel),
        groq_api_key: groqApiKey,
        groq_model: groqModel || DEFAULT_GROQ_MODEL,
        auto_ocr: autoOcr,
        system_prompt: systemPrompt,
      };

      const updated = await chatService.saveConfig(payload);
      if (updated) {
        config = updated;
        geminiApiKey = '';
        groqApiKey = '';
      }

      await voiceSectionRef?.saveVoiceConfig();
      window.dispatchEvent(new CustomEvent('undetect:config-updated'));

      saveFeedback = 'Settings saved successfully.';
      showToast('Settings saved successfully');
      setTimeout(() => {
        saveFeedback = '';
      }, 3000);
    } catch (e) {
      saveFeedback = 'Error saving settings: ' + (e.message || e);
    } finally {
      saving = false;
    }
  }
</script>

<div class="flex-1 flex flex-col min-h-0 overflow-clip bg-zinc-950 font-sans">
  <!-- Fixed Top Settings Header -->
  <div class="px-4 py-2.5 bg-zinc-950 border-b border-zinc-800/90 grid grid-cols-[1fr_auto_1fr] items-center gap-2 shrink-0 select-none">
    <div class="justify-self-start">
      <Button
        color="alternative"
        size="xs"
        onclick={onClose}
        class="h-7 px-2.5 py-0 rounded-lg bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 dark:hover:bg-zinc-800 border-zinc-800 dark:border-zinc-800 text-zinc-300 dark:text-zinc-300 hover:text-white dark:hover:text-white text-xs font-medium inline-flex items-center gap-1.5 leading-none cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
      >
        <ArrowLeftOutline class="w-3.5 h-3.5 shrink-0" />
        <span class="leading-none">Back</span>
      </Button>
    </div>

    <span class="text-xs font-semibold text-zinc-100 tracking-wide justify-self-center">Settings</span>

    <div class="justify-self-end">
      <Button
        color="alternative"
        size="xs"
        onclick={onCheckUpdates}
        title="Check for updates (v{status.app_version})"
        class="h-7 px-2.5 py-0 rounded-lg bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 dark:hover:bg-zinc-800 border-zinc-800 dark:border-zinc-800 text-zinc-300 dark:text-zinc-300 hover:text-white dark:hover:text-white text-[11px] font-sans font-medium inline-flex items-center gap-1.5 leading-none cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
      >
        <RefreshOutline class="w-3 h-3 shrink-0" />
        <span class="leading-none whitespace-nowrap">Check Updates</span>
        {#if updateBanner.show}
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
        {/if}
      </Button>
    </div>
  </div>

  <!-- Category Sub-Tabs Bar (Eliminates Vertical Information Overload) -->
  <div class="px-4 py-2 bg-zinc-950 border-b border-zinc-800/80 shrink-0 select-none overflow-x-auto">
    <div class="flex items-center gap-1 p-1 rounded-xl bg-zinc-900/90 border border-zinc-800/90 w-full">
      {#each SECTIONS as section (section.id)}
        {@const isActive = activeSection === section.id}
        <button
          type="button"
          onclick={() => selectSection(section.id)}
          class="flex-1 min-w-0 px-2.5 py-1.5 rounded-lg text-[11px] font-sans font-medium whitespace-nowrap transition-colors cursor-pointer {isActive ? 'bg-white text-zinc-950 font-semibold shadow-sm' : 'text-zinc-400 hover:text-zinc-100 hover:bg-zinc-800/70'}"
        >
          {section.label}
        </button>
      {/each}
    </div>
  </div>

  <!-- Scrollable Category Content -->
  <div
    data-scroll-feed
    bind:this={settingsScrollContainer}
    class="flex-1 overflow-y-auto px-4 py-3.5 flex flex-col gap-3 w-full box-border select-none"
  >
    {#if updateBanner.show}
      <div class="p-3 rounded-xl bg-zinc-900/90 border border-zinc-700/80 flex items-center justify-between gap-2 text-xs">
        <div>
          <div class="text-white font-semibold">v{updateBanner.version} Available</div>
          <div class="text-[11px] text-zinc-400">Ready to download and install.</div>
        </div>
        <Button
          color="alternative"
          size="xs"
          disabled={isUpdating}
          onclick={onDownloadAndInstallUpdate}
          class="h-7 px-3 py-0 rounded-lg bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-none text-xs font-semibold cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
        >
          <span class="text-zinc-950">{isUpdating ? 'Updating...' : 'Update Now'}</span>
        </Button>
      </div>
    {/if}

    {#if saveFeedback}
      <Alert
        color="gray"
        dismissable
        onclose={() => (saveFeedback = '')}
        class="bg-zinc-900 dark:bg-zinc-900 border border-zinc-700/80 dark:border-zinc-700/80 text-zinc-100 dark:text-zinc-100 text-xs py-2.5 px-3 rounded-xl"
      >
        {#snippet icon()}
          <CheckOutline class="w-4 h-4 text-emerald-400 shrink-0" />
        {/snippet}
        <span class="font-medium">{saveFeedback}</span>
      </Alert>
    {/if}

    <!-- Tab 1: AI Model Provider -->
    <div class={activeSection === 'model' ? 'flex flex-col gap-3' : 'hidden'}>
      <Card
        size="xl"
        color="gray"
        class="w-full bg-zinc-900/45 dark:bg-zinc-900/45 border border-zinc-800/90 dark:border-zinc-800/90 rounded-2xl p-4 flex flex-col gap-3 shadow-sm"
      >
        <div class="flex items-center justify-between pb-2.5 border-b border-zinc-800/80">
          <div class="flex items-center gap-2 text-[13px] font-semibold text-zinc-100">
            <AdjustmentsVerticalOutline class="w-4 h-4 text-zinc-400 shrink-0" />
            <span>Model Provider</span>
          </div>
          <Badge color="gray" class="bg-zinc-800 dark:bg-zinc-800 text-zinc-200 dark:text-zinc-200 border border-zinc-700/70 dark:border-zinc-700/70 rounded-full text-[11px] font-sans font-medium px-2.5 py-0.5 leading-none inline-flex items-center gap-1.5">
            <span class="w-1.5 h-1.5 rounded-full {modelKeyReady ? 'bg-emerald-400' : 'bg-zinc-500'}"></span>
            <span>{modelKeyReady ? (activeProvider === 'groq' ? 'Groq Ready' : 'Gemini Ready') : 'Key Required'}</span>
          </Badge>
        </div>

        <ModelProviderFields
          idPrefix="settings"
          bind:activeProvider={activeProvider}
          bind:geminiApiKey={geminiApiKey}
          bind:geminiModel={geminiModel}
          bind:groqApiKey={groqApiKey}
          bind:groqModel={groqModel}
          bind:autoOcr={autoOcr}
          bind:systemPrompt={systemPrompt}
          groqModelItems={groqModelItems}
          loadingGroqModels={loadingGroqModels}
          bind:config={config}
          ocrScreenshotId="latest"
          chatService={chatService}
          onRefreshGroqModels={fetchGroqModels}
          onNotify={(msg) => showToast(msg)}
        />
      </Card>
    </div>

    <!-- Tab 2: Voice Input -->
    <div class={activeSection === 'voice' ? 'flex flex-col gap-3' : 'hidden'}>
      <VoiceSettingsCard bind:this={voiceSectionRef} showToast={showToast} />
    </div>

    <!-- Tab 3: Window & Privacy -->
    <div class={activeSection === 'window' ? 'flex flex-col gap-3' : 'hidden'}>
      <Card
        size="xl"
        color="gray"
        class="w-full bg-zinc-900/45 dark:bg-zinc-900/45 border border-zinc-800/90 dark:border-zinc-800/90 rounded-2xl p-4 flex flex-col gap-2.5 shadow-sm"
      >
        <div class="flex items-center gap-2 pb-2.5 border-b border-zinc-800/80 text-[13px] font-semibold text-zinc-100">
          <ShieldCheckOutline class="w-4 h-4 text-zinc-400 shrink-0" />
          <span>Window &amp; Privacy</span>
        </div>

        <!-- Transparency Slider -->
        <div class="w-full p-3.5 bg-zinc-950/75 border border-zinc-800/80 rounded-xl flex flex-col gap-2">
          <div class="flex items-center justify-between gap-2">
            <div class="text-[12.5px] font-medium text-zinc-100">Transparency</div>
            <Badge color="gray" class="bg-zinc-800 dark:bg-zinc-800 border border-zinc-700/70 dark:border-zinc-700/70 text-zinc-100 dark:text-zinc-100 rounded-full text-[11px] font-sans font-medium tabular-nums px-2.5 py-0.5 leading-none shrink-0">
              {windowOpacity}%
            </Badge>
          </div>

          <Range
            id="window-opacity-slider"
            min={40}
            max={100}
            step={1}
            size="sm"
            appearance="auto"
            color="gray"
            bind:value={windowOpacity}
            oninput={() => onSetWindowOpacity(windowOpacity)}
            class="w-full bg-zinc-800 dark:bg-zinc-800 accent-white rounded-lg cursor-pointer"
          />

          <div class="flex items-center justify-between text-[10.5px] font-sans tabular-nums text-zinc-500">
            <span>40% Translucent</span>
            <span>100% Solid</span>
          </div>
        </div>

        <!-- Window Size -->
        <div class="w-full p-3.5 bg-zinc-950/75 border border-zinc-800/80 rounded-xl flex flex-col gap-2.5">
          <div class="flex items-center justify-between gap-2">
            <div class="text-[12.5px] font-medium text-zinc-100">Window Size</div>
            <Badge color="gray" class="bg-zinc-800 dark:bg-zinc-800 border border-zinc-700/70 dark:border-zinc-700/70 text-zinc-100 dark:text-zinc-100 rounded-full text-[11px] font-sans font-medium tabular-nums px-2.5 py-0.5 leading-none shrink-0">
              {windowSize.width} × {windowSize.height}
            </Badge>
          </div>

          <div class="flex flex-wrap items-center gap-1.5">
            <Button
              color="alternative"
              size="xs"
              onclick={() => onStepResize(-60, 0)}
              class="h-7 px-2.5 py-0 rounded-lg bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 dark:hover:bg-zinc-800 border-zinc-800 dark:border-zinc-800 text-zinc-300 dark:text-zinc-300 hover:text-white text-[11px] font-sans font-medium cursor-pointer focus:ring-0"
              title="Narrower width (-60 px)"
            >
              − Width
            </Button>

            <Button
              color="alternative"
              size="xs"
              onclick={() => onStepResize(60, 0)}
              class="h-7 px-2.5 py-0 rounded-lg bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 dark:hover:bg-zinc-800 border-zinc-800 dark:border-zinc-800 text-zinc-300 dark:text-zinc-300 hover:text-white text-[11px] font-sans font-medium cursor-pointer focus:ring-0"
              title="Wider width (+60 px)"
            >
              + Width
            </Button>

            {#each WINDOW_SIZE_PRESETS as preset (preset.id)}
              {@const isActive = Math.abs(windowSize.width - preset.width) <= 8 && Math.abs(windowSize.height - preset.height) <= 8}
              <Button
                color="alternative"
                size="xs"
                onclick={() => (preset.id === 'default' ? onResetWindowSize() : onSetWindowSize(preset.width, preset.height))}
                class="h-7 px-2.5 py-0 rounded-lg text-[11px] font-sans font-medium cursor-pointer focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0 {isActive ? 'bg-white dark:bg-white text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 border-white dark:border-white hover:bg-zinc-200 dark:hover:bg-zinc-200' : 'bg-zinc-900/60 dark:bg-zinc-900/60 hover:bg-zinc-800 dark:hover:bg-zinc-800 border-zinc-800 dark:border-zinc-800 text-zinc-400 dark:text-zinc-400 hover:text-white dark:hover:text-white'}"
                title="{preset.width} × {preset.height}"
              >
                <span class={isActive ? 'text-zinc-950' : ''}>{preset.label}</span>
              </Button>
            {/each}
          </div>
        </div>

        <!-- Window Toggles -->
        <div class="w-full flex items-center justify-between gap-3 p-3.5 bg-zinc-950/75 border border-zinc-800/80 rounded-xl">
          <div class="min-w-0">
            <div class="text-[12.5px] font-medium text-zinc-100">Always on Top</div>
            <div class="text-[11px] text-zinc-400 leading-snug mt-0.5">Keep pinned above other windows.</div>
          </div>
          <Toggle
            color="gray"
            size="small"
            class="relative cursor-pointer"
            bind:checked={alwaysOnTop}
            onchange={(e) => onToggleAlwaysOnTop(e.currentTarget.checked)}
            classes={monoToggleClasses}
            aria-label="Always appear on top"
          />
        </div>

        <div class="w-full flex items-center justify-between gap-3 p-3.5 bg-zinc-950/75 border border-zinc-800/80 rounded-xl">
          <div class="min-w-0">
            <div class="text-[12.5px] font-medium text-zinc-100">Exclude from Screenshots</div>
            <div class="text-[11px] text-zinc-400 leading-snug mt-0.5">Hide window from screen captures &amp; sharing.</div>
          </div>
          <Toggle
            color="gray"
            size="small"
            class="relative cursor-pointer"
            bind:checked={contentProtected}
            onchange={(e) => onToggleProtection(e.currentTarget.checked)}
            classes={monoToggleClasses}
            aria-label="Exclude window from screenshots"
          />
        </div>

        <div class="w-full flex items-center justify-between gap-3 p-3.5 bg-zinc-950/75 border border-zinc-800/80 rounded-xl">
          <div class="min-w-0">
            <div class="text-[12.5px] font-medium text-zinc-100">Hide from Taskbar</div>
            <div class="text-[11px] text-zinc-400 leading-snug mt-0.5">Hide Undetect icon from Windows taskbar.</div>
          </div>
          <Toggle
            color="gray"
            size="small"
            class="relative cursor-pointer"
            bind:checked={skipTaskbar}
            onchange={(e) => onToggleTaskbar(e.currentTarget.checked)}
            classes={monoToggleClasses}
            aria-label="Hide from Windows taskbar"
          />
        </div>

        <div class="w-full flex items-center justify-between gap-3 p-3.5 bg-zinc-950/75 border border-zinc-800/80 rounded-xl">
          <div class="min-w-0">
            <div class="text-[12.5px] font-medium text-zinc-100">Ghost Mode (Click-Through)</div>
            <div class="text-[11px] text-zinc-400 leading-snug mt-0.5">Pass clicks through window; press Ctrl + Alt + G to toggle.</div>
          </div>
          <Toggle
            color="gray"
            size="small"
            class="relative cursor-pointer"
            bind:checked={clickThrough}
            onchange={(e) => onToggleClickThrough(e.currentTarget.checked)}
            classes={monoToggleClasses}
            aria-label="Enable click-through ghost mode"
          />
        </div>

        <div class="w-full flex items-center justify-between gap-3 p-3.5 bg-zinc-950/75 border border-zinc-800/80 rounded-xl">
          <div class="min-w-0">
            <div class="text-[12.5px] font-medium text-zinc-100">Neutral Cursor</div>
            <div class="text-[11px] text-zinc-400 leading-snug mt-0.5">Keep default arrow cursor instead of resize/hand pointers on hover.</div>
          </div>
          <Toggle
            color="gray"
            size="small"
            class="relative cursor-pointer"
            checked={stealthCursor}
            onchange={(e) => onToggleStealthCursor(e.currentTarget.checked)}
            classes={monoToggleClasses}
            aria-label="Keep neutral OS cursor"
          />
        </div>

        <div class="w-full flex items-center justify-between gap-3 p-3.5 bg-zinc-950/75 border border-zinc-800/80 rounded-xl">
          <div class="min-w-0">
            <div class="text-[12.5px] font-medium text-zinc-100">Usage Stats</div>
            <div class="text-[11px] text-zinc-400 leading-snug mt-0.5">Share anonymous usage stats.</div>
          </div>
          <Toggle
            color="gray"
            size="small"
            class="relative cursor-pointer"
            bind:checked={telemetryEnabled}
            onchange={(e) => onToggleTelemetry(e.currentTarget.checked)}
            classes={monoToggleClasses}
            aria-label="Share anonymous usage stats"
          />
        </div>
      </Card>
    </div>

    <!-- Tab 4: Remote Server & App Updates -->
    <div class={activeSection === 'server' ? 'flex flex-col gap-3' : 'hidden'}>
      <Card
        size="xl"
        color="gray"
        class="w-full bg-zinc-900/45 dark:bg-zinc-900/45 border border-zinc-800/90 dark:border-zinc-800/90 rounded-2xl p-4 flex flex-col gap-3 shadow-sm"
      >
        <div class="flex items-center justify-between pb-2.5 border-b border-zinc-800/80">
          <div class="flex items-center gap-2 text-[13px] font-semibold text-zinc-100">
            <DesktopPcOutline class="w-4 h-4 text-zinc-400 shrink-0" />
            <span>Remote View Server</span>
          </div>
          <Badge color="gray" class="bg-zinc-800 dark:bg-zinc-800 border border-zinc-700/70 dark:border-zinc-700/70 text-zinc-200 dark:text-zinc-200 rounded-full text-[11px] font-sans font-medium px-2.5 py-0.5 leading-none inline-flex items-center gap-1.5">
            <span class="w-1.5 h-1.5 rounded-full {status.running ? 'bg-emerald-400 shadow-[0_0_6px_rgba(52,211,153,0.6)]' : 'bg-zinc-500'}"></span>
            <span>{status.running ? 'Running' : 'Stopped'}</span>
          </Badge>
        </div>

        <div class="grid grid-cols-1 sm:grid-cols-2 gap-3">
          <div class="flex flex-col gap-1.5">
            <Label for="settings-port" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">Server Port</Label>
            <Input
              id="settings-port"
              type="number"
              bind:value={serverPort}
              placeholder="9090"
              size="sm"
              class={monoInputClass}
            />
          </div>

          <div class="flex flex-col gap-1.5">
            <Label for="settings-pass" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">Access Passcode (PIN)</Label>
            <Input
              id="settings-pass"
              type="password"
              bind:value={serverPassword}
              placeholder="Set PIN"
              size="sm"
              class={monoInputClass}
            />
          </div>
        </div>
      </Card>
    </div>

    <!-- Tab 5: Keyboard Shortcuts -->
    {#snippet shortcutRow(sc, muted = false)}
      <div class="py-2.5 flex items-center justify-between gap-3 text-xs">
        <span class="{muted ? 'text-zinc-400' : 'text-zinc-200'} leading-snug">{sc.label}</span>
        <div class="flex items-center gap-1 shrink-0">
          {#each sc.keys as keycap, i (i)}
            {#if i > 0}
              <span class="text-zinc-600 text-[10px] font-sans">+</span>
            {/if}
            <kbd class="min-w-6 text-center px-1.5 py-0.5 rounded-md bg-zinc-900 border border-zinc-700/80 {muted ? 'text-zinc-300' : 'text-zinc-100'} font-mono text-[10px] leading-tight">{keycap}</kbd>
          {/each}
        </div>
      </div>
    {/snippet}

    <div class={activeSection === 'shortcuts' ? 'flex flex-col gap-3' : 'hidden'}>
      <Card
        size="xl"
        color="gray"
        class="w-full bg-zinc-900/45 dark:bg-zinc-900/45 border border-zinc-800/90 dark:border-zinc-800/90 rounded-2xl p-4 flex flex-col gap-3 shadow-sm"
      >
        <div class="flex items-center justify-between pb-2.5 border-b border-zinc-800/80">
          <div class="flex items-center gap-2 text-[13px] font-semibold text-zinc-100">
            <KeyboardOutline class="w-4 h-4 text-zinc-400 shrink-0" />
            <span>Keyboard Shortcuts</span>
          </div>
          <span class="text-[11px] font-sans text-zinc-400">One key per action</span>
        </div>

        <div class="flex flex-col gap-3">
          <div class="flex flex-col gap-1.5">
            <div class="px-0.5 text-[11px] font-semibold text-zinc-400 tracking-wide uppercase">Works Anywhere</div>
            <div class="flex flex-col divide-y divide-zinc-800/70 rounded-xl bg-zinc-950/75 border border-zinc-800/80 px-3.5">
              {#each GLOBAL_SHORTCUTS as sc (sc.label)}
                {@render shortcutRow(sc)}
              {/each}
            </div>
          </div>

          <div class="flex flex-col gap-1.5">
            <div class="px-0.5 text-[11px] font-semibold text-zinc-400 tracking-wide uppercase">When App Is Focused</div>
            <div class="flex flex-col divide-y divide-zinc-800/70 rounded-xl bg-zinc-950/75 border border-zinc-800/80 px-3.5">
              {#each IN_APP_SHORTCUTS as sc (sc.label)}
                {@render shortcutRow(sc)}
              {/each}
            </div>
          </div>

          <Button
            color="alternative"
            size="xs"
            aria-expanded={showMoreShortcuts}
            onclick={() => (showMoreShortcuts = !showMoreShortcuts)}
            class="w-full justify-between px-3.5 py-2 rounded-xl bg-zinc-950/75 dark:bg-zinc-950/75 hover:bg-zinc-900 dark:hover:bg-zinc-900 border-zinc-800/80 dark:border-zinc-800/80 text-zinc-400 dark:text-zinc-400 hover:text-zinc-200 text-xs font-sans font-medium cursor-pointer focus:ring-0"
          >
            <span>More global shortcuts</span>
            {#if showMoreShortcuts}
              <ChevronDownOutline class="w-3.5 h-3.5 text-zinc-400" />
            {:else}
              <ChevronRightOutline class="w-3.5 h-3.5 text-zinc-400" />
            {/if}
          </Button>

          {#if showMoreShortcuts}
            <div class="flex flex-col divide-y divide-zinc-800/70 rounded-xl bg-zinc-950/75 border border-zinc-800/80 px-3.5">
              {#each MORE_SHORTCUTS as sc (sc.label)}
                {@render shortcutRow(sc, true)}
              {/each}
            </div>
          {/if}
        </div>
      </Card>
    </div>
  </div>

  <!-- Fixed Bottom Save Bar (Always in One Place!) -->
  <div class="px-4 py-3 bg-zinc-950/95 border-t border-zinc-800/90 shrink-0 select-none">
    <Button
      color="alternative"
      class="w-full h-10 py-0 bg-white dark:bg-white hover:bg-zinc-200 dark:hover:bg-zinc-200 active:bg-zinc-300 text-zinc-950 dark:text-zinc-950 hover:text-zinc-950 dark:hover:text-zinc-950 focus-within:text-zinc-950 dark:focus-within:text-zinc-950 font-sans font-semibold text-xs rounded-xl shadow-lg transition-all flex items-center justify-center gap-2 cursor-pointer border-none focus:ring-0 focus-within:ring-0 dark:focus-within:ring-0"
      disabled={saving}
      onclick={handleSaveAll}
    >
      <CheckOutline class="w-4 h-4 text-zinc-950 shrink-0" />
      <span class="text-zinc-950">{saving ? 'Saving Settings...' : 'Save All Settings'}</span>
    </Button>
  </div>
</div>
