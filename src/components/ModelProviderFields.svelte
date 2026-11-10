<script>
  import {
    Badge,
    Button,
    Dropdown,
    DropdownGroup,
    DropdownItem,
    Input,
    Label,
    Textarea,
    Toggle
  } from 'flowbite-svelte';
  import {
    CheckOutline,
    ChevronDownOutline,
    ChevronRightOutline,
    FileCheckOutline,
    RefreshOutline,
    SearchOutline,
    TrashBinOutline,
    UploadOutline
  } from 'flowbite-svelte-icons';
  import {
    ChatService,
    DEFAULT_GEMINI_MODEL,
    DEFAULT_GROQ_MODEL,
    DEFAULT_INTERVIEW_SYSTEM_PROMPT,
    GEMINI_MODEL_ITEMS,
    PROVIDER_ITEMS
  } from '../chat/services/ChatService.js';
  import {
    DROPDOWN_CLASS,
    DROPDOWN_GROUP_CLASS,
    getDropdownItemClass,
    syncDropdownWidth
  } from '../lib/dropdownClasses.js';

  export let idPrefix = 'settings';
  export let activeProvider = 'groq';
  export let geminiApiKey = '';
  export let geminiModel = DEFAULT_GEMINI_MODEL;
  export let groqApiKey = '';
  export let groqModel = DEFAULT_GROQ_MODEL;
  export let groqModelItems = [];
  export let loadingGroqModels = false;
  export let autoOcr = true;
  export let systemPrompt = '';
  export let config = {
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
  export let ocrScreenshotId = 'latest';
  export let chatService = new ChatService();
  export let onRefreshGroqModels = () => {};
  export let onNotify = () => {};

  let providerDropdownOpen = false;
  let modelDropdownOpen = false;
  let showAdvanced = false;
  let showResumePreview = false;
  let clearingKey = false;
  let uploadingResume = false;
  let clearingResume = false;
  let resumeError = '';
  let resumeInputRef;

  let ocrLoading = false;
  let ocrTestText = '';
  let ocrTestError = '';

  const monoInputClass =
    'w-full bg-zinc-950 dark:bg-zinc-950 border-zinc-800 dark:border-zinc-800 text-zinc-100 dark:text-zinc-100 placeholder-zinc-500 dark:placeholder-zinc-500 text-xs font-sans focus:border-white dark:focus:border-white focus:ring-0 dark:focus:ring-0 rounded-xl';

  const monoToggleClasses = {
    span: 'me-0 border-0 ring-1 ring-inset ring-zinc-700 bg-zinc-800 dark:bg-zinc-800 after:border-0 after:top-[2px] after:start-[2px] after:h-4 after:w-4 after:bg-zinc-300 peer-checked:bg-white dark:peer-checked:bg-white peer-checked:ring-white peer-checked:after:bg-zinc-950 peer-focus:ring-0 dark:peer-focus:ring-0',
  };

  $: selectedProviderObj =
    PROVIDER_ITEMS.find((p) => p.value === activeProvider) || PROVIDER_ITEMS[0];

  $: currentModelItems =
    activeProvider === 'groq'
      ? (groqModelItems.length > 0
          ? groqModelItems
          : [{ value: groqModel || DEFAULT_GROQ_MODEL, name: groqModel || DEFAULT_GROQ_MODEL, tag: 'Vision' }])
      : GEMINI_MODEL_ITEMS;

  $: selectedModelValue = activeProvider === 'groq' ? groqModel : geminiModel;
  $: selectedModelObj =
    currentModelItems.find((m) => m.value === selectedModelValue) || {
      value: selectedModelValue,
      name: selectedModelValue || 'Select model...',
      tag: '',
    };

  function handleSelectProvider(providerId) {
    activeProvider = providerId;
    providerDropdownOpen = false;
    if (providerId === 'groq' && groqModelItems.length === 0) {
      onRefreshGroqModels();
    }
  }

  function handleSelectModel(modelId) {
    if (activeProvider === 'groq') {
      groqModel = modelId;
    } else {
      geminiModel = modelId;
    }
    modelDropdownOpen = false;
  }

  async function handleClearApiKey(providerId) {
    if (clearingKey) return;
    clearingKey = true;
    try {
      const updated = await chatService.clearApiKey(providerId);
      config = { ...config, ...updated };
      if (providerId === 'gemini') {
        geminiApiKey = '';
      } else if (providerId === 'groq') {
        groqApiKey = '';
      }
      window.dispatchEvent(new CustomEvent('undetect:config-updated'));
      onNotify(`${providerId === 'gemini' ? 'Gemini' : 'Groq'} API key cleared.`, 'success');
    } catch (e) {
      onNotify(e?.message || String(e), 'error');
    } finally {
      clearingKey = false;
    }
  }

  async function handleTestOcr() {
    if (ocrLoading) return;
    ocrLoading = true;
    ocrTestError = '';
    try {
      const text = await chatService.runOcr(ocrScreenshotId || 'latest');
      ocrTestText = text || '(No readable text detected on screen)';
    } catch (e) {
      ocrTestText = '';
      ocrTestError = e.message || String(e);
    } finally {
      ocrLoading = false;
    }
  }

  async function handleResumeFileChange(ev) {
    const file = ev.currentTarget?.files?.[0];
    if (ev.currentTarget) ev.currentTarget.value = '';
    if (!file || uploadingResume) return;

    uploadingResume = true;
    resumeError = '';
    try {
      const updated = await chatService.uploadResume(file);
      config = { ...config, ...updated };
      window.dispatchEvent(new CustomEvent('undetect:config-updated'));
      onNotify(`Resume loaded (${updated.resume_pages} ${updated.resume_pages === 1 ? 'page' : 'pages'}).`, 'success');
    } catch (e) {
      resumeError = e?.message || String(e);
      onNotify(resumeError, 'error');
    } finally {
      uploadingResume = false;
    }
  }

  async function handleClearResume() {
    if (clearingResume) return;
    clearingResume = true;
    resumeError = '';
    try {
      const updated = await chatService.clearResume();
      config = { ...config, ...updated };
      showResumePreview = false;
      window.dispatchEvent(new CustomEvent('undetect:config-updated'));
      onNotify('Resume cleared.', 'success');
    } catch (e) {
      onNotify(e?.message || String(e), 'error');
    } finally {
      clearingResume = false;
    }
  }
</script>

<div class="flex flex-col gap-3 w-full">
  <!-- Active Provider Flowbite Dropdown -->
  <div class="flex flex-col gap-1.5">
    <Label for="{idPrefix}-provider-btn" class="text-xs font-medium text-zinc-400 dark:text-zinc-400">Provider</Label>
    <Button
      id="{idPrefix}-provider-btn"
      color="alternative"
      size="sm"
      class="w-full justify-between px-3 py-2 bg-zinc-950 dark:bg-zinc-950 hover:bg-zinc-900 dark:hover:bg-zinc-900 border border-zinc-800 dark:border-zinc-800 hover:border-zinc-600 text-zinc-100 dark:text-zinc-100 text-xs font-sans font-medium rounded-xl focus:ring-0 cursor-pointer"
    >
      <span class="pointer-events-none flex items-center gap-2 min-w-0 truncate font-sans">
        <span class="font-semibold text-zinc-100">{selectedProviderObj.name}</span>
        <Badge color="gray" class="bg-zinc-800 dark:bg-zinc-800 text-zinc-300 dark:text-zinc-300 border border-zinc-700 dark:border-zinc-700 rounded-full text-[10px] font-sans font-medium px-2 py-0.5 leading-none">
          {selectedProviderObj.tag}
        </Badge>
      </span>
      <ChevronDownOutline class="w-3.5 h-3.5 text-zinc-400 shrink-0 pointer-events-none" />
    </Button>

    <Dropdown
      bind:isOpen={providerDropdownOpen}
      triggeredBy="#{idPrefix}-provider-btn"
      placement="bottom-start"
      strategy="fixed"
      triggerDelay={0}
      offset={4}
      onbeforetoggle={syncDropdownWidth}
      class={DROPDOWN_CLASS}
    >
      <DropdownGroup class={DROPDOWN_GROUP_CLASS}>
        {#each PROVIDER_ITEMS as item (item.value)}
          <DropdownItem
            onclick={() => handleSelectProvider(item.value)}
            class={getDropdownItemClass(activeProvider === item.value)}
          >
            <div class="flex items-center gap-2 min-w-0">
              <span class="truncate">{item.name}</span>
              <Badge color="gray" class="bg-zinc-900 dark:bg-zinc-900 border border-zinc-700 dark:border-zinc-700 text-zinc-400 dark:text-zinc-400 rounded-full text-[10px] font-sans font-medium px-2 py-0.5 leading-none">
                {item.tag}
              </Badge>
            </div>
            {#if activeProvider === item.value}
              <CheckOutline class="w-3.5 h-3.5 text-white shrink-0" />
            {/if}
          </DropdownItem>
        {/each}
      </DropdownGroup>
    </Dropdown>
  </div>

  <!-- Selected Provider Configuration Only (Clean & Uncluttered) -->
  <div class="p-3.5 rounded-xl bg-zinc-950/75 border border-zinc-800/80 flex flex-col gap-3">
    {#if activeProvider === 'gemini'}
      <div class="flex items-center justify-between">
        <span class="text-xs font-semibold text-zinc-200">Google Gemini Setup</span>
        <a
          href="https://aistudio.google.com/app/apikey"
          target="_blank"
          class="text-[11px] font-sans font-medium text-zinc-400 hover:text-white underline underline-offset-2"
        >
          Get API Key ↗
        </a>
      </div>

      <div class="flex flex-col gap-1.5">
        <div class="flex items-center justify-between gap-2">
          <Label for="{idPrefix}-gemini-key" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">API Key</Label>
          {#if config.has_gemini_key}
            <button
              type="button"
              disabled={clearingKey}
              onclick={() => handleClearApiKey('gemini')}
              class="inline-flex items-center gap-1 text-[11px] font-medium text-zinc-400 hover:text-white disabled:opacity-50 cursor-pointer transition-colors"
              title="Clear saved Gemini API key"
            >
              <TrashBinOutline class="w-3 h-3" />
              <span>{clearingKey ? 'Clearing...' : 'Clear saved key'}</span>
            </button>
          {/if}
        </div>
        <Input
          id="{idPrefix}-gemini-key"
          type="password"
          bind:value={geminiApiKey}
          placeholder={config.has_gemini_key ? `Saved (${config.gemini_key_masked})` : 'Paste Gemini API key...'}
          size="sm"
          class={monoInputClass}
        />
        {#if !config.has_gemini_key && !geminiApiKey.trim()}
          <p class="text-[11px] text-zinc-400 leading-snug">
            No Gemini key saved. AI chat won't work until you add a key and save.
          </p>
        {/if}
      </div>
    {:else}
      <div class="flex items-center justify-between">
        <span class="text-xs font-semibold text-zinc-200">Groq Cloud Setup</span>
        <a
          href="https://console.groq.com/keys"
          target="_blank"
          class="text-[11px] font-sans font-medium text-zinc-400 hover:text-white underline underline-offset-2"
        >
          Get API Key ↗
        </a>
      </div>

      <div class="flex flex-col gap-1.5">
        <div class="flex items-center justify-between gap-2">
          <Label for="{idPrefix}-groq-key" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">API Key</Label>
          {#if config.has_groq_key}
            <button
              type="button"
              disabled={clearingKey}
              onclick={() => handleClearApiKey('groq')}
              class="inline-flex items-center gap-1 text-[11px] font-medium text-zinc-400 hover:text-white disabled:opacity-50 cursor-pointer transition-colors"
              title="Clear saved Groq API key"
            >
              <TrashBinOutline class="w-3 h-3" />
              <span>{clearingKey ? 'Clearing...' : 'Clear saved key'}</span>
            </button>
          {/if}
        </div>
        <Input
          id="{idPrefix}-groq-key"
          type="password"
          bind:value={groqApiKey}
          placeholder={config.has_groq_key ? `Saved (${config.groq_key_masked})` : 'Paste Groq API key (gsk_...)'}
          size="sm"
          class={monoInputClass}
          onblur={onRefreshGroqModels}
        />
        {#if !config.has_groq_key && !groqApiKey.trim()}
          <p class="text-[11px] text-zinc-400 leading-snug">
            No Groq key saved. AI chat won't work until you add a key and save.
          </p>
        {/if}
      </div>
    {/if}

    <!-- Shared Model Dropdown Button (kept mounted cleanly while provider switches) -->
    <div class="flex flex-col gap-1.5">
      <div class="flex items-center justify-between gap-2">
        <Label for="{idPrefix}-model-btn" class="text-[11px] font-medium text-zinc-400 dark:text-zinc-400">Model</Label>
        {#if activeProvider === 'groq'}
          <Button
            color="alternative"
            size="xs"
            class="h-6 px-2 py-0 text-[11px] font-sans font-medium bg-zinc-900 dark:bg-zinc-900 text-zinc-300 dark:text-zinc-300 hover:text-white border-zinc-800 dark:border-zinc-800 hover:bg-zinc-800 rounded-lg focus:ring-0 inline-flex items-center gap-1 cursor-pointer"
            onclick={onRefreshGroqModels}
            disabled={loadingGroqModels}
            title="Sync available Groq models"
          >
            <RefreshOutline class="w-3 h-3 {loadingGroqModels ? 'animate-spin' : ''}" />
            <span>{loadingGroqModels ? 'Syncing...' : 'Refresh'}</span>
          </Button>
        {/if}
      </div>

      <Button
        id="{idPrefix}-model-btn"
        color="alternative"
        size="sm"
        class="w-full justify-between px-3 py-2 bg-zinc-950 dark:bg-zinc-950 hover:bg-zinc-900 dark:hover:bg-zinc-900 border border-zinc-800 dark:border-zinc-800 hover:border-zinc-600 text-zinc-100 dark:text-zinc-100 text-xs font-sans rounded-xl focus:ring-0 cursor-pointer"
      >
        <span class="pointer-events-none flex items-center gap-2 min-w-0">
          <span class="truncate font-medium">{selectedModelObj.name}</span>
          {#if selectedModelObj.tag}
            <Badge color="gray" class="bg-zinc-800 dark:bg-zinc-800 text-zinc-300 dark:text-zinc-300 border border-zinc-700 dark:border-zinc-700 rounded-full text-[10px] font-sans font-medium px-2 py-0.5 leading-none shrink-0">
              {selectedModelObj.tag}
            </Badge>
          {/if}
        </span>
        <ChevronDownOutline class="w-3.5 h-3.5 text-zinc-400 shrink-0 pointer-events-none" />
      </Button>

      <Dropdown
        bind:isOpen={modelDropdownOpen}
        triggeredBy="#{idPrefix}-model-btn"
        placement="bottom-start"
        strategy="fixed"
        triggerDelay={0}
        offset={4}
        onbeforetoggle={syncDropdownWidth}
        class={DROPDOWN_CLASS}
      >
        <DropdownGroup class={DROPDOWN_GROUP_CLASS}>
          {#each currentModelItems as item (item.value)}
            <DropdownItem
              onclick={() => handleSelectModel(item.value)}
              class={getDropdownItemClass(selectedModelValue === item.value)}
            >
              <div class="flex items-center gap-2 min-w-0">
                <span class="truncate">{item.name}</span>
                {#if item.tag}
                  <Badge color="gray" class="bg-zinc-900 dark:bg-zinc-900 border border-zinc-700 dark:border-zinc-700 text-zinc-400 dark:text-zinc-400 rounded-full text-[10px] font-sans font-medium px-2 py-0.5 leading-none shrink-0">
                    {item.tag}
                  </Badge>
                {/if}
              </div>
              {#if selectedModelValue === item.value}
                <CheckOutline class="w-3.5 h-3.5 text-white shrink-0" />
              {/if}
            </DropdownItem>
          {/each}
        </DropdownGroup>
      </Dropdown>
    </div>
  </div>

  <!-- Interview Resume Upload (Max 2 Pages, Grounds Interview System Prompt) -->
  <input
    bind:this={resumeInputRef}
    type="file"
    accept=".pdf,.docx,.docm,.odt,.rtf,.txt,.md,.doc"
    onchange={handleResumeFileChange}
    class="hidden"
  />

  <div class="p-3.5 rounded-xl bg-zinc-950/75 border border-zinc-800/80 flex flex-col gap-2.5">
    <div class="flex items-center justify-between gap-2">
      <div class="min-w-0">
        <div class="text-xs font-semibold text-zinc-200">Interview Resume </div>
        <div class="text-[11px] text-zinc-400 leading-snug mt-0.5">
          .pdf, .docx, RTF, or TXT.
        </div>
      </div>

      <Badge color="gray" class="bg-zinc-900 dark:bg-zinc-900 border border-zinc-700/70 dark:border-zinc-700/70 text-zinc-200 dark:text-zinc-200 rounded-full text-[10.5px] font-sans font-medium px-2.5 py-0.5 leading-none inline-flex items-center gap-1.5 shrink-0">
        <span class="w-1.5 h-1.5 rounded-full {config.has_resume ? 'bg-emerald-400' : 'bg-zinc-500'}"></span>
        <span>
          {config.has_resume
            ? `${config.resume_pages || 1} ${config.resume_pages === 1 ? 'pg' : 'pgs'}`
            : 'Optional'}
        </span>
      </Badge>
    </div>

    {#if config.has_resume}
      <div class="p-2.5 rounded-xl bg-zinc-900/70 border border-zinc-800 flex items-center justify-between gap-2">
        <div class="flex items-center gap-2 min-w-0">
          <FileCheckOutline class="w-4 h-4 text-emerald-400 shrink-0" />
          <div class="min-w-0">
            <div class="text-xs font-medium text-zinc-100 truncate">{config.resume_filename}</div>
            <div class="text-[10.5px] text-zinc-400 tabular-nums">
              {config.resume_chars?.toLocaleString?.() || config.resume_chars} chars extracted
            </div>
          </div>
        </div>

        <div class="flex items-center gap-1.5 shrink-0">
          <Button
            color="alternative"
            size="xs"
            disabled={uploadingResume}
            onclick={() => resumeInputRef?.click()}
            class="h-7 px-2.5 py-0 rounded-lg bg-zinc-950 dark:bg-zinc-950 hover:bg-zinc-800 text-zinc-200 dark:text-zinc-200 hover:text-white border-zinc-800 dark:border-zinc-800 text-[11px] font-medium cursor-pointer focus:ring-0"
          >
            {uploadingResume ? 'Reading...' : 'Replace'}
          </Button>
          <Button
            color="alternative"
            size="xs"
            disabled={clearingResume}
            onclick={handleClearResume}
            class="h-7 px-2 py-0 rounded-lg bg-transparent dark:bg-transparent hover:bg-zinc-800 text-zinc-400 dark:text-zinc-400 hover:text-white border-zinc-800 dark:border-zinc-800 text-[11px] font-medium cursor-pointer focus:ring-0"
            title="Remove resume context"
          >
            <TrashBinOutline class="w-3.5 h-3.5" />
          </Button>
        </div>
      </div>

      {#if config.resume_preview}
        <button
          type="button"
          onclick={() => (showResumePreview = !showResumePreview)}
          class="inline-flex items-center gap-1.5 text-[11px] font-medium text-zinc-400 hover:text-zinc-200 cursor-pointer self-start transition-colors"
        >
          {#if showResumePreview}
            <ChevronDownOutline class="w-3 h-3" />
          {:else}
            <ChevronRightOutline class="w-3 h-3" />
          {/if}
          <span>{showResumePreview ? 'Hide extracted text' : 'Preview extracted text'}</span>
        </button>

        {#if showResumePreview}
          <div class="rounded-lg border border-zinc-800 bg-zinc-950 overflow-hidden p-2">
            <pre class="max-h-32 overflow-y-auto pr-1 font-mono text-[11px] text-zinc-300 whitespace-pre-wrap select-text leading-relaxed">{config.resume_preview}</pre>
          </div>
        {/if}
      {/if}
    {:else}
      <Button
        color="alternative"
        size="xs"
        disabled={uploadingResume}
        onclick={() => resumeInputRef?.click()}
        class="w-full h-8 px-3 py-0 rounded-xl bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 dark:hover:bg-zinc-800 text-zinc-200 dark:text-zinc-200 hover:text-white border border-zinc-800 dark:border-zinc-800 text-xs font-sans font-medium inline-flex items-center justify-center gap-2 cursor-pointer focus:ring-0"
      >
        <UploadOutline class="w-3.5 h-3.5 text-zinc-300 shrink-0" />
        <span>{uploadingResume ? 'Extracting Resume...' : 'Upload Resume'}</span>
      </Button>
    {/if}

    {#if resumeError}
      <p class="text-[11px] text-zinc-200 bg-zinc-900 border border-zinc-700 rounded-lg px-2.5 py-2 leading-snug">
        {resumeError}
      </p>
    {/if}
  </div>

  <!-- Advanced (OCR & Instructions) Disclosure -->
  <div class="pt-1">
    <button
      type="button"
      onclick={() => (showAdvanced = !showAdvanced)}
      class="inline-flex items-center gap-1.5 text-[11px] font-medium text-zinc-400 hover:text-zinc-100 cursor-pointer transition-colors"
    >
      {#if showAdvanced}
        <ChevronDownOutline class="w-3.5 h-3.5" />
      {:else}
        <ChevronRightOutline class="w-3.5 h-3.5" />
      {/if}
      <span>Advanced (OCR &amp; System Prompt)</span>
    </button>

    {#if showAdvanced}
      <div class="mt-2.5 flex flex-col gap-3">
        <!-- Windows On-Device OCR -->
        <div class="p-3.5 rounded-xl bg-zinc-950/75 border border-zinc-800/80 flex flex-col gap-2.5">
          <div class="flex items-center justify-between gap-3">
            <div class="min-w-0">
              <div class="text-[12.5px] font-medium text-zinc-100">Windows On-Device OCR</div>
              <div class="text-[11px] text-zinc-400 leading-snug mt-0.5">Extract screen text locally when attaching captures.</div>
            </div>
            <Toggle
              color="gray"
              size="small"
              class="relative cursor-pointer"
              bind:checked={autoOcr}
              classes={monoToggleClasses}
              aria-label="Enable Windows on-device OCR"
            />
          </div>

          <div class="pt-2 border-t border-zinc-800/80 flex items-center justify-between gap-2">
            <Button
              color="alternative"
              size="xs"
              class="h-7 py-0 px-2.5 bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 text-zinc-300 dark:text-zinc-300 hover:text-white border-zinc-800 dark:border-zinc-800 rounded-lg text-[11px] font-sans font-medium inline-flex items-center gap-1.5 focus:ring-0 cursor-pointer"
              disabled={ocrLoading}
              onclick={handleTestOcr}
            >
              <SearchOutline class="w-3 h-3 text-zinc-400" />
              <span>{ocrLoading ? 'Running OCR...' : 'Test OCR on Screen'}</span>
            </Button>

            {#if ocrTestText}
              <Button
                color="alternative"
                size="xs"
                class="h-7 py-0 px-2.5 bg-zinc-900 dark:bg-zinc-900 hover:bg-zinc-800 text-zinc-300 dark:text-zinc-300 hover:text-white border-zinc-800 dark:border-zinc-800 rounded-lg text-[11px] font-sans font-medium focus:ring-0 cursor-pointer"
                onclick={() => navigator.clipboard.writeText(ocrTestText)}
              >
                Copy Text
              </Button>
            {/if}
          </div>

          {#if ocrTestError}
            <p class="text-[11px] font-sans text-zinc-400 bg-zinc-950 border border-zinc-800 rounded-lg p-2.5">{ocrTestError}</p>
          {:else if ocrTestText}
            <div class="rounded-lg border border-zinc-800 bg-zinc-950 overflow-hidden p-2">
              <pre class="max-h-24 overflow-y-auto pr-1 font-mono text-[11px] text-zinc-300 whitespace-pre-wrap select-text">{ocrTestText}</pre>
            </div>
          {/if}
        </div>

        <!-- Custom System Prompt -->
        <div class="flex flex-col gap-1.5 w-full">
          <div class="flex items-center justify-between gap-2">
            <Label for="{idPrefix}-system-prompt" class="text-xs font-medium text-zinc-400 dark:text-zinc-400">
              Interview System Instructions
            </Label>
            {#if systemPrompt.trim() !== DEFAULT_INTERVIEW_SYSTEM_PROMPT}
              <button
                type="button"
                onclick={() => (systemPrompt = DEFAULT_INTERVIEW_SYSTEM_PROMPT)}
                class="text-[11px] font-medium text-zinc-400 hover:text-white cursor-pointer transition-colors"
              >
                Reset default
              </button>
            {/if}
          </div>
          <div class="w-full rounded-xl border border-zinc-800 bg-zinc-950 overflow-hidden focus-within:border-white transition-colors p-2.5">
            <Textarea
              id="{idPrefix}-system-prompt"
              bind:value={systemPrompt}
              rows="4"
              placeholder={DEFAULT_INTERVIEW_SYSTEM_PROMPT}
              classes={{ div: 'w-full' }}
              class="w-full max-h-36 overflow-y-auto bg-transparent dark:bg-transparent border-none dark:border-none text-zinc-100 dark:text-zinc-100 placeholder-zinc-500 dark:placeholder-zinc-500 text-xs font-sans p-0 pr-1 rounded-none focus:ring-0 dark:focus:ring-0 resize-none leading-relaxed shadow-none"
            />
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>
