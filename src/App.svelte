<script>
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { InfoCircleOutline } from 'flowbite-svelte-icons';

  import TitleBar from './components/TitleBar.svelte';
  import ServerConfigCard from './components/ServerConfigCard.svelte';
  import ProtectionCard from './components/ProtectionCard.svelte';
  import LanAccessCard from './components/LanAccessCard.svelte';
  import ActionButtons from './components/ActionButtons.svelte';
  import QrModal from './components/QrModal.svelte';
  import WhatsNewModal from './components/WhatsNewModal.svelte';
  import Toast from './components/Toast.svelte';

  let status = {
    running: false,
    port: 9090,
    password: '1234',
    local_urls: [],
    screenshot_count: 0,
    skip_taskbar: false,
    content_protected: true,
    app_version: '2.0.0',
  };

  let qrModalOpen = false;
  let currentQrData = '';
  let currentQrTarget = '';

  let whatsNewOpen = false;

  function closeWhatsNew() {
    whatsNewOpen = false;
    try {
      localStorage.setItem('undetect_seen_version', status.app_version || '2.0.0');
    } catch (e) {}
  }

  function checkWhatsNew() {
    try {
      const seen = localStorage.getItem('undetect_seen_version');
      if (seen !== '2.0.0') {
        whatsNewOpen = true;
      }
    } catch (e) {}
  }

  let toastMessage = '';
  let toastVisible = false;
  let toastTimer = null;

  let updateBanner = {
    show: false,
    version: '',
    downloadUrl: '',
  };

  let refreshInterval = null;

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
      showToast('Server Updated & Started!');
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

  async function handleCapture() {
    showToast('Capturing screen...');
    try {
      const meta = await invoke('capture_now');
      showToast(`Captured at ${meta.timestamp}`);
      await refreshStatus();
    } catch (e) {
      showToast('Capture error: ' + e);
    }
  }

  async function handleClear() {
    try {
      await invoke('clear_session');
      showToast('Session Cleared');
      await refreshStatus();
    } catch (e) {
      showToast('Clear error: ' + e);
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

  async function handleOpenQr(url) {
    currentQrTarget = url;
    currentQrData = '';
    qrModalOpen = true;
    try {
      const dataUrl = await invoke('generate_qr', { text: url });
      currentQrData = dataUrl;
    } catch (e) {
      showToast('Failed to generate QR: ' + e);
    }
  }

  function handleCopyUrl(url) {
    navigator.clipboard.writeText(url);
    showToast('URL Copied to Clipboard!');
  }

  async function checkUpdates(manual = false) {
    if (manual) showToast('Checking GitHub for updates...');
    try {
      const res = await fetch('https://api.github.com/repos/zigglyzuff/undetect/releases/latest');
      if (res.ok) {
        const release = await res.json();
        const latestTag = release.tag_name ? release.tag_name.replace(/^v/, '') : '';
        if (latestTag && latestTag !== status.app_version) {
          const exeAsset = release.assets ? release.assets.find(a => a.name.endsWith('.exe')) : null;
          updateBanner = {
            show: true,
            version: latestTag,
            downloadUrl: exeAsset ? exeAsset.browser_download_url : release.html_url,
          };
          showToast(`New Version v${latestTag} Available!`);
          return;
        }
      }
      if (manual) showToast(`Latest version (v${status.app_version}) is installed`);
    } catch (e) {
      if (manual) showToast('Update check failed');
    }
  }

  onMount(() => {
    refreshStatus();
    checkWhatsNew();
    refreshInterval = setInterval(refreshStatus, 3000);
  });

  onDestroy(() => {
    if (refreshInterval) clearInterval(refreshInterval);
  });
</script>

<main class="w-full h-full bg-[#090a0f] text-zinc-100 flex flex-col overflow-hidden relative select-none">
  <!-- Draggable Custom Title Bar -->
  <TitleBar
    version={`v${status.app_version}`}
    onCheckUpdates={() => checkUpdates(true)}
  />

  <!-- Scrollable App Content Container - Balanced & Full Width -->
  <div class="flex-1 overflow-y-auto px-4 py-3.5 flex flex-col gap-3 w-full box-border">
    {#if updateBanner.show}
      <div class="w-full bg-zinc-900 border border-zinc-700/80 text-zinc-200 p-3 rounded-xl flex items-center justify-between gap-3 text-xs shadow-lg">
        <div class="flex items-center gap-2">
          <InfoCircleOutline class="w-4 h-4 text-zinc-300 shrink-0" />
          <div>
            <b class="text-white font-semibold">New Release Ready: v{updateBanner.version}</b>
            <div class="text-[11px] text-zinc-400">An update package is available on GitHub.</div>
          </div>
        </div>
        <a
          href={updateBanner.downloadUrl}
          target="_blank"
          class="font-semibold text-[11px] px-3 py-1.5 rounded-lg bg-white hover:bg-zinc-200 text-zinc-950 transition-colors shadow shrink-0"
        >
          Download
        </a>
      </div>
    {/if}

    <!-- Server Configuration Card -->
    <ServerConfigCard
      running={status.running}
      port={status.port}
      password={status.password}
      onStart={handleStartServer}
      onStop={handleStopServer}
    />

    <!-- Window Protection & Taskbar Card -->
    <ProtectionCard
      contentProtected={status.content_protected}
      skipTaskbar={status.skip_taskbar}
      onToggleProtection={handleToggleProtection}
      onToggleTaskbar={handleToggleTaskbar}
    />

    <!-- Local Wi-Fi Access URLs Card -->
    <LanAccessCard
      localUrls={status.local_urls}
      onOpenQr={handleOpenQr}
      onCopyUrl={handleCopyUrl}
    />

    <!-- Quick Action Buttons -->
    <ActionButtons
      count={status.screenshot_count}
      onCapture={handleCapture}
      onClear={handleClear}
    />
  </div>

  <!-- QR Scan Modal -->
  <QrModal
    isOpen={qrModalOpen}
    qrDataUrl={currentQrData}
    targetUrl={currentQrTarget}
    onClose={() => qrModalOpen = false}
  />

  <!-- What's New in v2.0 Modal (Shows once on update) -->
  <WhatsNewModal
    isOpen={whatsNewOpen}
    onClose={closeWhatsNew}
  />

  <!-- Feedback Toast -->
  <Toast
    message={toastMessage}
    visible={toastVisible}
  />
</main>
