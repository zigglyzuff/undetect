<script>
  import { onMount, onDestroy, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { check } from '@tauri-apps/plugin-updater';
  import { relaunch } from '@tauri-apps/plugin-process';
  import { Tabs, TabItem } from 'flowbite-svelte';
  import { DesktopPcOutline, MessagesOutline } from 'flowbite-svelte-icons';

  import TitleBar from './components/TitleBar.svelte';
  import WhatsNewModal from './components/WhatsNewModal.svelte';
  import Toast from './components/Toast.svelte';

  import ChatView from './chat/ChatView.svelte';
  import RemoteServerView from './components/RemoteServerView.svelte';
  import SettingsView from './components/SettingsView.svelte';

  let activeTab = 'chat'; // Default view is Chat screen
  let settingsOpen = false;
  let settingsSection = 'model';
  let attachScreenshot = true;
  let chatViewRef;
  let settingsViewRef;

  let windowSize = { width: 460, height: 710 };

  const RESIZE_HANDLES = [
    { dir: 'n', cls: 'top-0 left-2 right-2 h-1 cursor-ns-resize' },
    { dir: 's', cls: 'bottom-0 left-2 right-2 h-1 cursor-ns-resize' },
    { dir: 'w', cls: 'left-0 top-2 bottom-2 w-1 cursor-ew-resize' },
    { dir: 'e', cls: 'right-0 top-2 bottom-2 w-1 cursor-ew-resize' },
    { dir: 'nw', cls: 'top-0 left-0 w-2 h-2 cursor-nwse-resize' },
    { dir: 'ne', cls: 'top-0 right-0 w-2 h-2 cursor-nesw-resize' },
    { dir: 'sw', cls: 'bottom-0 left-0 w-2 h-2 cursor-nesw-resize' },
    { dir: 'se', cls: 'bottom-0 right-0 w-2 h-2 cursor-nwse-resize' },
  ];

  let status = {
    running: false,
    port: 9090,
    password: '1234',
    local_urls: [],
    screenshot_count: 0,
    skip_taskbar: false,
    content_protected: true,
    always_on_top: true,
    click_through: false,
    window_opacity: 100,
    telemetry_enabled: true,
    app_version: '2.2.0',
    ui_version: null,
  };

  let stealthCursor = true;

  let whatsNewOpen = false;

  function majorMinorKey(ver) {
    const raw = String(ver || '').trim();
    if (!raw) return '';
    const clean = raw.replace(/^v/, '').split('-')[0];
    const parts = clean.split('.');
    return parts.length >= 2 ? `${parts[0]}.${parts[1]}` : clean;
  }

  function closeWhatsNew() {
    whatsNewOpen = false;
    try {
      localStorage.setItem('undetect_seen_version', majorMinorKey(status.app_version || '2.2.0'));
    } catch (e) {}
  }

  function checkWhatsNew() {
    try {
      const currentSeries = majorMinorKey(status.app_version || '2.2.0');
      const seen = localStorage.getItem('undetect_seen_version');
      if (majorMinorKey(seen) !== currentSeries) {
        whatsNewOpen = true;
      }
    } catch (e) {}
  }

  let toastMessage = '';
  let toastVisible = false;
  let toastTimer = null;

  let pendingBinaryUpdate = null;
  let updateBanner = {
    show: false,
    version: '',
  };

  let refreshInterval = null;
  let resizePersistTimer = null;
  let unlistenFns = [];

  function persistWindowSize(size) {
    if (!size) return;
    try {
      localStorage.setItem('undetect_window_size', JSON.stringify(size));
    } catch (e) {}
  }

  async function syncWindowSize() {
    try {
      const size = await invoke('get_window_size');
      if (size) windowSize = size;
    } catch (e) {}
  }

  async function handleSetWindowSize(width, height) {
    try {
      const applied = await invoke('set_window_size', {
        width: Number(width),
        height: Number(height),
      });
      if (applied) {
        windowSize = applied;
        persistWindowSize(applied);
        showToast(`Window: ${applied.width} × ${applied.height}`);
      }
    } catch (e) {
      showToast('Resize error: ' + e);
    }
  }

  async function handleStepResize(dw, dh) {
    try {
      const applied = await invoke('step_resize_window', { dw, dh });
      if (applied) {
        windowSize = applied;
        persistWindowSize(applied);
        showToast(`Window: ${applied.width} × ${applied.height}`);
      }
    } catch (e) {
      showToast('Resize error: ' + e);
    }
  }

  async function handleResetWindowSize() {
    try {
      const applied = await invoke('reset_window_size');
      if (applied) {
        windowSize = applied;
        persistWindowSize(applied);
        showToast(`Window reset (${applied.width} × ${applied.height})`);
      }
    } catch (e) {
      showToast('Reset error: ' + e);
    }
  }

  async function handleNudgeWindow(dx, dy) {
    try {
      await invoke('nudge_window', { dx, dy });
    } catch (e) {}
  }

  function handleStartResizeDrag(e, direction) {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    invoke('start_resize_drag', { direction }).catch(console.error);
  }

  function handleWindowResized() {
    if (resizePersistTimer) clearTimeout(resizePersistTimer);
    resizePersistTimer = setTimeout(async () => {
      try {
        const size = await invoke('get_window_size');
        if (size) {
          windowSize = size;
          persistWindowSize(size);
        }
      } catch (e) {}
    }, 180);
  }

  function showToast(msg) {
    if (toastTimer) clearTimeout(toastTimer);
    toastMessage = msg;
    toastVisible = true;
    toastTimer = setTimeout(() => {
      toastVisible = false;
    }, 2200);
  }

  async function refreshStatus() {
    try {
      const res = await invoke('get_status');
      if (res) {
        status = res;
      }
    } catch (e) {
      console.error('Failed to get status:', e);
    }
  }

  async function handleStartServer(port, password) {
    try {
      const res = await invoke('start_server', { port: Number(port), password });
      status = res;
      showToast('Server Started!');
    } catch (e) {
      showToast('Error: ' + e);
    }
  }

  async function handleStopServer() {
    try {
      const res = await invoke('stop_server');
      status = res;
      showToast('Server Stopped');
    } catch (e) {
      showToast('Error: ' + e);
    }
  }

  async function handleToggleProtection(protect) {
    try {
      const res = await invoke('toggle_content_protection', { protect });
      status.content_protected = res;
      showToast(res ? 'Window Protected from Screen Capture' : 'Protection Disabled');
    } catch (e) {
      showToast('Protection error: ' + e);
    }
  }

  async function handleToggleTaskbar(hide) {
    try {
      const res = await invoke('toggle_taskbar', { hide });
      status.skip_taskbar = res;
      showToast(res ? 'Hidden from Taskbar' : 'Restored to Taskbar');
    } catch (e) {
      showToast('Taskbar error: ' + e);
    }
  }

  async function handleToggleAlwaysOnTop(alwaysOnTop) {
    try {
      const res = await invoke('toggle_always_on_top', { alwaysOnTop });
      status.always_on_top = res;
      try {
        localStorage.setItem('undetect_always_on_top', String(res));
      } catch (e) {}
      showToast(res ? 'Pinned Always on Top' : 'Unpinned from Top');
    } catch (e) {
      showToast('Always on top error: ' + e);
    }
  }

  async function handleToggleTelemetry(enabled) {
    try {
      const res = await invoke('toggle_telemetry', { enabled });
      status.telemetry_enabled = res;
      showToast(res ? 'Anonymous usage stats enabled' : 'Anonymous usage stats disabled');
    } catch (e) {
      showToast('Telemetry setting error: ' + e);
    }
  }

  async function handleToggleClickThrough(clickThrough) {
    try {
      const res = await invoke('toggle_click_through', { clickThrough });
      status.click_through = res;
      showToast(res ? 'Ghost Mode ON (Ctrl + Alt + G to interact)' : 'Ghost Mode OFF');
    } catch (e) {
      showToast('Ghost mode error: ' + e);
    }
  }

  function handleToggleStealthCursor(enabled) {
    stealthCursor = Boolean(enabled);
    try {
      localStorage.setItem('undetect_stealth_cursor', String(stealthCursor));
    } catch (e) {}
    showToast(stealthCursor ? 'Neutral Cursor enabled' : 'Standard cursors restored');
  }

  async function handleSetWindowOpacity(opacity) {
    const target = Math.max(40, Math.min(100, Math.round(Number(opacity) || 100)));
    try {
      const applied = await invoke('set_window_opacity', { opacity: target });
      status.window_opacity = applied;
      try {
        localStorage.setItem('undetect_window_opacity', String(applied));
      } catch (e) {}
    } catch (e) {
      showToast('Opacity error: ' + e);
    }
  }

  async function restoreWindowPreferences() {
    try {
      const savedOpacity = Number(localStorage.getItem('undetect_window_opacity'));
      if (!Number.isNaN(savedOpacity) && savedOpacity >= 40 && savedOpacity <= 100) {
        const applied = await invoke('set_window_opacity', { opacity: Math.round(savedOpacity) });
        status.window_opacity = applied;
      }

      const savedTop = localStorage.getItem('undetect_always_on_top');
      if (savedTop !== null) {
        const topBool = savedTop !== 'false';
        const appliedTop = await invoke('toggle_always_on_top', { alwaysOnTop: topBool });
        status.always_on_top = appliedTop;
      }

      const savedCursor = localStorage.getItem('undetect_stealth_cursor');
      if (savedCursor !== null) {
        stealthCursor = savedCursor !== 'false';
      }

      const rawSize = localStorage.getItem('undetect_window_size');
      if (rawSize) {
        const parsed = JSON.parse(rawSize);
        if (parsed && Number(parsed.width) >= 360 && Number(parsed.height) >= 480) {
          const appliedSize = await invoke('set_window_size', {
            width: Number(parsed.width),
            height: Number(parsed.height),
          });
          if (appliedSize) windowSize = appliedSize;
        }
      } else {
        await syncWindowSize();
      }
    } catch (e) {
      console.warn('Could not restore window preferences:', e);
    }
  }

  async function focusChatInputSoon() {
    await tick();
    requestAnimationFrame(() => chatViewRef?.focusInput());
  }

  function scrollActiveFeed(deltaY) {
    const feeds = document.querySelectorAll('[data-scroll-feed]');
    for (const el of feeds) {
      if (el instanceof HTMLElement && el.offsetParent !== null) {
        el.scrollTop += deltaY;
        return true;
      }
    }
    if (settingsOpen) {
      settingsViewRef?.scrollContent(deltaY);
      return true;
    }
    if (activeTab === 'chat') {
      chatViewRef?.scrollMessages(deltaY);
      return true;
    }
    return false;
  }

  async function focusChatView() {
    whatsNewOpen = false;
    settingsOpen = false;
    activeTab = 'chat';
    await focusChatInputSoon();
  }

  function openSettings(section = 'model') {
    if (section) settingsSection = section;
    settingsOpen = true;
  }

  async function closeSettings() {
    settingsOpen = false;
    await chatViewRef?.refreshAvailability?.();
    if (activeTab === 'chat') {
      await focusChatInputSoon();
    }
  }

  async function handleGlobalKeydown(e) {
    const active = document.activeElement;
    const tag = active?.tagName?.toLowerCase();
    const isEditable = tag === 'input' || tag === 'textarea' || tag === 'select' || active?.isContentEditable;
    const ctrl = (e.ctrlKey || e.metaKey) && !e.altKey;

    if (e.key === 'Escape') {
      if (whatsNewOpen) {
        e.preventDefault();
        closeWhatsNew();
      } else if (settingsOpen) {
        e.preventDefault();
        await closeSettings();
      } else if (isEditable) {
        active?.blur();
      }
      return;
    }

    if (e.altKey && !e.shiftKey && !e.ctrlKey && !e.metaKey) {
      if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        e.preventDefault();
        scrollActiveFeed(e.key === 'ArrowUp' ? -150 : 150);
        return;
      }
      if (!settingsOpen && !whatsNewOpen && activeTab === 'chat') {
        if (e.key === 'Enter') {
          e.preventDefault();
          chatViewRef?.sendVoiceTranscript?.();
          return;
        }
        if (e.key === 'Backspace' && chatViewRef?.clearVoiceTranscript?.()) {
          e.preventDefault();
          return;
        }
      }
    }

    if (!e.ctrlKey && !e.altKey && !e.metaKey && (e.key === 'PageUp' || e.key === 'PageDown')) {
      e.preventDefault();
      scrollActiveFeed(e.key === 'PageUp' ? -240 : 240);
      return;
    }

    // Core focused shortcuts
    if (ctrl && !e.shiftKey) {
      if (e.code === 'KeyK') {
        e.preventDefault();
        await focusChatView();
        return;
      }
      if (e.code === 'KeyJ' && !settingsOpen && !whatsNewOpen && activeTab === 'chat') {
        e.preventDefault();
        await chatViewRef?.openJdModal?.();
        return;
      }
      if (e.key === ',' || e.code === 'Comma') {
        e.preventDefault();
        if (settingsOpen) {
          await closeSettings();
        } else {
          openSettings('model');
        }
        return;
      }
      if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
        e.preventDefault();
        scrollActiveFeed(e.key === 'ArrowUp' ? -150 : 150);
        return;
      }
      if (e.code === 'BracketLeft') {
        e.preventDefault();
        handleStepResize(-60, 0);
        return;
      }
      if (e.code === 'BracketRight') {
        e.preventDefault();
        handleStepResize(60, 0);
        return;
      }
      if (e.code === 'Digit0' || e.code === 'Numpad0' || e.key === '0') {
        e.preventDefault();
        handleResetWindowSize();
        return;
      }
    }
  }

  let isUpdating = false;

  async function handleDownloadAndInstallUpdate() {
    if (!pendingBinaryUpdate) {
      showToast('No update package available');
      return;
    }

    if (isUpdating) return;
    isUpdating = true;
    showToast('Downloading and verifying update...');

    try {
      await pendingBinaryUpdate.downloadAndInstall();
      await relaunch();
    } catch (e) {
      console.error('Update failed:', e);
      showToast('Update failed: ' + e);
      isUpdating = false;
    }
  }

  async function checkUiPatch(manual = false) {
    try {
      const res = await invoke('check_and_stage_ui_patch');
      if (res?.staged && manual) {
        showToast(`UI patch ${res.pending_ui_version} staged — applies on restart`);
        return true;
      }
      if (res?.pending_ui_version && manual) {
        showToast(`UI patch ${res.pending_ui_version} already staged — restart to apply`);
        return true;
      }
    } catch (e) {
      console.warn('UI patch check skipped:', e);
    }
    return false;
  }

  async function checkUpdates(manual = false) {
    if (manual) showToast('Checking for updates...');
    try {
      const update = await check();
      if (update?.available) {
        pendingBinaryUpdate = update;
        updateBanner = {
          show: true,
          version: update.version,
        };
        showToast(`New Version v${update.version} Available!`);
        return;
      }
      pendingBinaryUpdate = null;
      updateBanner = { show: false, version: '' };

      const stagedUi = await checkUiPatch(manual);
      if (manual && !stagedUi) {
        showToast(`Latest version (v${status.app_version}) is installed`);
      }
    } catch (e) {
      console.warn('Binary update check failed:', e);
      const stagedUi = await checkUiPatch(manual);
      if (manual && !stagedUi) {
        showToast(`Latest version (v${status.app_version}) is installed`);
      }
    }
  }

  function setupClientErrorReporting() {
    const handleWindowError = (event) => {
      try {
        const message = String(event?.message || event?.error?.message || event || 'Unknown JS Error');
        const source = event?.filename || '';
        const lineno = event?.lineno || 0;
        const colno = event?.colno || 0;
        const stack = event?.error?.stack || '';
        invoke('report_client_error', {
          errorType: 'uncaught_error',
          message,
          source,
          lineno,
          colno,
          stack,
        }).catch(() => {});
      } catch (e) {}
    };

    const handleUnhandledRejection = (event) => {
      try {
        const reason = event?.reason;
        const message = typeof reason === 'string' ? reason : (reason?.message || 'Unhandled Promise Rejection');
        const stack = reason?.stack || '';
        invoke('report_client_error', {
          errorType: 'unhandled_rejection',
          message,
          source: '',
          lineno: 0,
          colno: 0,
          stack,
        }).catch(() => {});
      } catch (e) {}
    };

    window.addEventListener('error', handleWindowError);
    window.addEventListener('unhandledrejection', handleUnhandledRejection);
    return () => {
      window.removeEventListener('error', handleWindowError);
      window.removeEventListener('unhandledrejection', handleUnhandledRejection);
    };
  }

  onMount(async () => {
    const teardownErrorReporting = setupClientErrorReporting();
    unlistenFns.push(teardownErrorReporting);
    invoke('confirm_ui_boot').catch(() => {});
    await refreshStatus();
    await restoreWindowPreferences();
    checkWhatsNew();
    checkUpdates(false);
    refreshInterval = setInterval(refreshStatus, 3000);

    if (!whatsNewOpen && !settingsOpen && activeTab === 'chat') {
      await focusChatInputSoon();
    }

    document.addEventListener('keydown', handleGlobalKeydown, true);

    try {
      unlistenFns = await Promise.all([
        listen('shortcut://visibility', () => {
          // Intentionally avoid stealing focus when showing/hiding overlay
        }),
        listen('shortcut://focus-chat', async () => {
          await focusChatView();
        }),
        listen('shortcut://scroll', ({ payload }) => {
          scrollActiveFeed(Number(payload) || 0);
        }),
        listen('shortcut://toggle-screen', () => {
          attachScreenshot = !attachScreenshot;
          showToast(attachScreenshot ? 'Screen capture included' : 'Text-only mode (screen excluded)');
        }),
        listen('shortcut://protection', ({ payload }) => {
          status.content_protected = Boolean(payload);
          showToast(payload ? 'Window Protected from Screen Capture' : 'Protection Disabled');
        }),
        listen('shortcut://click-through', ({ payload }) => {
          status.click_through = Boolean(payload);
          showToast(payload ? 'Ghost Mode ON (Ctrl + Alt + G to exit)' : 'Ghost Mode OFF');
        }),
        listen('shortcut://answer-screen', async () => {
          whatsNewOpen = false;
          settingsOpen = false;
          activeTab = 'chat';
          await tick();
          chatViewRef?.answerScreen?.();
        }),
        listen('vosk://error', ({ payload }) => {
          if (payload) showToast(String(payload));
        }),
        listen('shortcut://resized', ({ payload }) => {
          if (payload) {
            windowSize = payload;
            persistWindowSize(payload);
            showToast(`Window: ${payload.width} × ${payload.height}`);
          }
        }),
      ]);
    } catch (e) {
      console.warn('Could not attach global shortcut event listeners:', e);
    }
  });

  onDestroy(() => {
    document.removeEventListener('keydown', handleGlobalKeydown, true);
    if (refreshInterval) clearInterval(refreshInterval);
    if (resizePersistTimer) clearTimeout(resizePersistTimer);
    for (const unlisten of unlistenFns) {
      if (typeof unlisten === 'function') unlisten();
    }
  });
</script>

<svelte:window onresize={handleWindowResized} />

<main class="fixed inset-0 w-full h-full bg-zinc-950 text-zinc-100 flex flex-col overflow-clip select-none {stealthCursor ? 'stealth-cursor' : ''}">
  <!-- Frameless Edge & Corner Resize Grips -->
  {#each RESIZE_HANDLES as handle (handle.dir)}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="fixed z-40 {handle.cls}"
      onmousedown={(e) => handleStartResizeDrag(e, handle.dir)}
    ></div>
  {/each}

  <!-- Draggable Custom Title Bar with Top-Right Settings Cogwheel -->
  <TitleBar
    version={`v${status.app_version}`}
    uiVersion={status.ui_version}
    settingsOpen={settingsOpen}
    clickThrough={status.click_through}
    onToggleSettings={() => (settingsOpen ? closeSettings() : openSettings('model'))}
  />

  {#if settingsOpen}
    <SettingsView
      bind:this={settingsViewRef}
      bind:activeSection={settingsSection}
      status={status}
      windowSize={windowSize}
      stealthCursor={stealthCursor}
      updateBanner={updateBanner}
      isUpdating={isUpdating}
      onClose={closeSettings}
      onSaveServer={handleStartServer}
      onToggleProtection={handleToggleProtection}
      onToggleTaskbar={handleToggleTaskbar}
      onToggleAlwaysOnTop={handleToggleAlwaysOnTop}
      onToggleClickThrough={handleToggleClickThrough}
      onToggleStealthCursor={handleToggleStealthCursor}
      onToggleTelemetry={handleToggleTelemetry}
      onSetWindowOpacity={handleSetWindowOpacity}
      onSetWindowSize={handleSetWindowSize}
      onStepResize={handleStepResize}
      onResetWindowSize={handleResetWindowSize}
      onCheckUpdates={() => checkUpdates(true)}
      onDownloadAndInstallUpdate={handleDownloadAndInstallUpdate}
      showToast={showToast}
    />
  {/if}

  <!-- Two-Tab Main Interface (Chat First & Default, Remote Server Second) -->
  <div class={settingsOpen ? 'hidden' : 'flex-1 min-h-0 flex flex-col overflow-clip'}>
    <Tabs
      bind:selected={activeTab}
      tabStyle="none"
      divider={false}
      class="px-3.5 pt-2 pb-1.5 bg-zinc-950 border-b border-zinc-800/80 flex items-center gap-1.5 space-x-0 shrink-0 select-none"
      classes={{ content: 'flex-1 min-h-0 flex flex-col overflow-hidden p-0 mt-0 bg-transparent dark:bg-transparent rounded-none' }}
    >
      <TabItem
        key="chat"
        open
        class="flex-1"
        activeClass="w-full h-8 px-3 rounded-lg text-xs font-bold leading-none flex items-center justify-center gap-2 bg-white text-zinc-950 shadow-md transition-all cursor-pointer focus:outline-none focus-visible:outline-1 focus-visible:outline-white/75"
        inactiveClass="w-full h-8 px-3 rounded-lg text-xs font-semibold leading-none flex items-center justify-center gap-2 bg-zinc-900/60 hover:bg-zinc-900 text-zinc-400 hover:text-zinc-200 border border-zinc-800 transition-all cursor-pointer focus:outline-none focus-visible:outline-1 focus-visible:outline-white/75"
      >
        {#snippet titleSlot()}
          <MessagesOutline class="w-3.5 h-3.5 shrink-0 block" />
          <span class="leading-none">Chat</span>
        {/snippet}

        <ChatView
          bind:this={chatViewRef}
          bind:attachScreenshot={attachScreenshot}
          currentScreenshotId="capture"
          authPin={status.password}
          onOpenSettings={openSettings}
          showToast={showToast}
        />
      </TabItem>

      <TabItem
        key="server"
        class="flex-1"
        activeClass="w-full h-8 px-3 rounded-lg text-xs font-bold leading-none flex items-center justify-center gap-2 bg-white text-zinc-950 shadow-md transition-all cursor-pointer focus:outline-none focus-visible:outline-1 focus-visible:outline-white/75"
        inactiveClass="w-full h-8 px-3 rounded-lg text-xs font-semibold leading-none flex items-center justify-center gap-2 bg-zinc-900/60 hover:bg-zinc-900 text-zinc-400 hover:text-zinc-200 border border-zinc-800 transition-all cursor-pointer focus:outline-none focus-visible:outline-1 focus-visible:outline-white/75"
      >
        {#snippet titleSlot()}
          <DesktopPcOutline class="w-3.5 h-3.5 shrink-0 block" />
          <span class="leading-none">Remote Server</span>
        {/snippet}

        <RemoteServerView
          running={status.running}
          port={status.port}
          password={status.password}
          localUrls={status.local_urls}
          onStart={handleStartServer}
          onStop={handleStopServer}
          onOpenSettings={() => openSettings('server')}
        />
      </TabItem>
    </Tabs>
  </div>

  <!-- What's New Modal (Shows once per release) -->
  <WhatsNewModal
    isOpen={whatsNewOpen}
    version={`v${status.app_version}`}
    onClose={closeWhatsNew}
  />

  <!-- Feedback Toast -->
  <Toast
    message={toastMessage}
    visible={toastVisible}
  />
</main>
