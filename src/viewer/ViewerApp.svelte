<script>
  import { onMount, onDestroy } from 'svelte';
  import ViewerNav from './components/ViewerNav.svelte';
  import ViewerPreview from './components/ViewerPreview.svelte';
  import ViewerAuthModal from './components/ViewerAuthModal.svelte';
  import ViewerToast from './components/ViewerToast.svelte';

  let currentPass = '';
  let status = 'online'; // 'online' | 'locked' | 'connecting'
  let screenshots = [];
  let currentSelectedId = null;

  let authOpen = false;
  let authError = false;

  let toastMessage = '';
  let toastVisible = false;
  let toastError = false;
  let toastTimer = null;

  let pollInterval = null;

  function showToast(msg, isError = false) {
    if (toastTimer) clearTimeout(toastTimer);
    toastMessage = msg;
    toastError = isError;
    toastVisible = true;
    toastTimer = setTimeout(() => {
      toastVisible = false;
    }, 2200);
  }

  function getAuthQuery() {
    return currentPass ? '?pass=' + encodeURIComponent(currentPass) : '';
  }

  async function loadSession(manual = false, fromAuth = false, forceLatest = false) {
    try {
      const url = '/api/session' + getAuthQuery();
      const res = await fetch(url);
      if (res.status === 403 || res.status === 401) {
        status = 'locked';
        authOpen = true;
        authError = fromAuth;
        return false;
      }
      if (!res.ok) throw new Error('Request status: ' + res.status);
      status = 'online';
      authOpen = false;
      authError = false;
      const data = await res.json();
      const newShots = data.screenshots || [];
      const prevLength = screenshots.length;
      const wasAtLatest = prevLength > 0 && currentSelectedId === screenshots[prevLength - 1]?.id;

      screenshots = newShots;

      // Selection logic:
      // 1. If forceLatest requested (e.g. after manual capture) -> select newest
      // 2. If no shot was selected yet -> select newest
      // 3. If currently selected shot was deleted -> select newest
      // 4. If user was viewing the latest shot and a new one arrived -> follow to newest
      if (screenshots.length > 0) {
        const selectedStillExists = currentSelectedId && screenshots.some(s => s.id === currentSelectedId);
        if (forceLatest || !currentSelectedId || !selectedStillExists || (wasAtLatest && screenshots.length > prevLength)) {
          currentSelectedId = screenshots[screenshots.length - 1].id;
        }
      } else {
        currentSelectedId = null;
      }

      if (manual) showToast('Refreshed session');
      return true;
    } catch (e) {
      console.error('Session load error:', e);
      status = 'connecting';
      return false;
    }
  }

  async function handleAuthSubmit(pass) {
    currentPass = pass;
    const ok = await loadSession(false, true);
    if (ok) {
      sessionStorage.setItem('undetect_pass', currentPass);
      showToast('Unlocked successfully');
    }
  }

  function handleSelectShot(id) {
    currentSelectedId = id;
  }

  // Reactive derived state with explicit dependencies for reliable Svelte reactivity
  $: currentShot = (screenshots && currentSelectedId)
    ? (screenshots.find(item => item.id === currentSelectedId) || null)
    : (screenshots && screenshots.length > 0 ? screenshots[screenshots.length - 1] : null);

  $: currentShotObj = currentShot
    ? {
        id: currentShot.id,
        timestamp: currentShot.timestamp,
        url: `/api/screenshot?id=${currentShot.id}${currentPass ? '&pass=' + encodeURIComponent(currentPass) : ''}`,
      }
    : null;

  $: currentIndex = (screenshots && currentSelectedId)
    ? screenshots.findIndex(s => s.id === currentSelectedId)
    : (screenshots && screenshots.length > 0 ? screenshots.length - 1 : -1);

  $: currentTimestamp = currentShotObj ? currentShotObj.timestamp : '';

  function handleDownloadCurrent() {
    const shot = currentShotObj;
    if (!shot) {
      showToast('No screenshot selected', true);
      return;
    }
    const timeStr = shot.timestamp.replace(/[: ]/g, '-');
    const a = document.createElement('a');
    a.href = shot.url;
    a.download = `undetect-${timeStr}.jpg`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    showToast('Downloading capture...');
  }

  async function handleBatchDownload() {
    if (!screenshots || screenshots.length === 0) {
      showToast('No screenshots to download', true);
      return;
    }
    showToast(`Starting batch download (${screenshots.length} images)...`);

    for (let i = 0; i < screenshots.length; i++) {
      const s = screenshots[i];
      const timeStr = s.timestamp.replace(/[: ]/g, '-');
      const url = `/api/screenshot?id=${s.id}${currentPass ? '&pass=' + encodeURIComponent(currentPass) : ''}`;

      try {
        const resp = await fetch(url);
        const blob = await resp.blob();
        const blobUrl = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = blobUrl;
        a.download = `undetect-${i + 1}-${timeStr}.jpg`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(blobUrl);
        await new Promise(r => setTimeout(r, 220));
      } catch (err) {
        console.error('Batch download item error:', err);
      }
    }
    showToast('Batch download finished!');
  }

  async function handleTriggerCapture() {
    showToast('Capturing screen...');
    try {
      const url = '/api/capture' + getAuthQuery();
      const res = await fetch(url, { method: 'POST' });
      const d = await res.json();
      if (d.success) {
        showToast('Captured successfully!');
        setTimeout(() => loadSession(false, false, true), 300);
      } else {
        showToast(d.message || 'Capture error', true);
      }
    } catch (e) {
      showToast('Failed to trigger capture', true);
    }
  }

  let clearConfirmOpen = false;

  function handlePromptClear() {
    if (!screenshots || screenshots.length === 0) return;
    clearConfirmOpen = true;
  }

  async function handleConfirmClear() {
    clearConfirmOpen = false;
    try {
      const url = '/api/session/clear' + getAuthQuery();
      await fetch(url, { method: 'POST' });
      currentSelectedId = null;
      showToast('Session captures cleared');
      loadSession();
    } catch (e) {
      showToast('Clear failed', true);
    }
  }

  onMount(() => {
    const urlParams = new URLSearchParams(window.location.search);
    currentPass = urlParams.get('pass') || urlParams.get('pin') || sessionStorage.getItem('undetect_pass') || '';
    
    loadSession();
    pollInterval = setInterval(() => loadSession(), 3000);
  });

  onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval);
  });
</script>

<div class="h-full h-[100dvh] w-full bg-[#090a0f] text-zinc-100 flex flex-col overflow-hidden fixed inset-0 select-none antialiased">
  <!-- Minimal Top Navbar (Clean Logo + Active Timestamp, Refined Live Pill) -->
  <ViewerNav
    count={screenshots.length}
    currentIndex={currentIndex}
    currentTimestamp={currentTimestamp}
    status={status}
  />

  <!-- Main Full-Screen Viewport (In-Place Zoom & Pan, Bottom Reel + Ergonomic Controls Dock) -->
  <main class="flex-1 w-full min-h-0 flex flex-col overflow-hidden m-0 p-0">
    <ViewerPreview
      currentShot={currentShotObj}
      screenshots={screenshots}
      currentSelectedId={currentSelectedId}
      currentPass={currentPass}
      onSelectShot={handleSelectShot}
      onCapture={handleTriggerCapture}
      onClear={handlePromptClear}
      onDownload={handleDownloadCurrent}
      onBatchDownload={handleBatchDownload}
    />
  </main>

  <!-- Clear Session Confirmation Modal -->
  {#if clearConfirmOpen}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4 animate-in fade-in duration-200 select-none"
      onclick={(e) => { if (e.target === e.currentTarget) clearConfirmOpen = false; }}
    >
      <div class="w-full max-w-[300px] bg-zinc-900 border border-white/10 rounded-2xl p-5 shadow-2xl flex flex-col items-center gap-3 text-center animate-in fade-in zoom-in-95 duration-200">
        <div class="w-10 h-10 rounded-full bg-rose-950/60 border border-rose-800/60 text-rose-400 flex items-center justify-center shadow-inner">
          <svg class="w-5 h-5" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"></path></svg>
        </div>
        <div>
          <h3 class="text-sm font-bold text-white">Clear Session History?</h3>
          <p class="text-xs text-zinc-400 mt-1">This will delete all {screenshots.length} captured screenshots from memory.</p>
        </div>
        <div class="grid grid-cols-2 gap-2 w-full mt-1">
          <button
            type="button"
            onclick={() => clearConfirmOpen = false}
            class="w-full py-2 px-3 rounded-lg bg-zinc-800 hover:bg-zinc-750 text-zinc-300 text-xs font-semibold transition-colors cursor-pointer active:scale-98"
          >
            Cancel
          </button>
          <button
            type="button"
            onclick={handleConfirmClear}
            class="w-full py-2 px-3 rounded-lg bg-rose-600 hover:bg-rose-500 active:bg-rose-700 text-white text-xs font-semibold transition-colors cursor-pointer shadow-md active:scale-98"
          >
            Clear All
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Authentication Modal (If host PIN is enabled) -->
  <ViewerAuthModal
    bind:open={authOpen}
    error={authError}
    onSubmit={handleAuthSubmit}
  />

  <!-- Feedback Toast -->
  <ViewerToast
    message={toastMessage}
    visible={toastVisible}
    isError={toastError}
  />
</div>
