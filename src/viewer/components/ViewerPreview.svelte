<script>
  import { onMount } from 'svelte';
  import ViewerFilmstrip from './ViewerFilmstrip.svelte';

  export let currentShot = null; // { id, timestamp, url }
  export let screenshots = [];
  export let currentSelectedId = null;
  export let currentPass = '';
  export let onSelectShot = (id, timestamp) => {};
  export let onCapture = () => {};
  export let onClear = () => {};
  export let onDownload = () => {};
  export let onBatchDownload = () => {};

  // In-place Zoom & Pan state
  let scale = 1;
  let translateX = 0;
  let translateY = 0;
  let isPanning = false;
  let startX = 0;
  let startY = 0;
  let initialDistance = 0;
  let initialScale = 1;
  let lastTap = 0;
  let isAnimating = false;
  let imageError = false;
  let lastShotId = null;

  $: currentIndex = screenshots.findIndex(s => s.id === currentSelectedId);

  function resetZoom() {
    scale = 1;
    translateX = 0;
    translateY = 0;
  }

  // Only reset zoom when switching to a DIFFERENT screenshot
  $: if (currentShot?.id && currentShot.id !== lastShotId) {
    lastShotId = currentShot.id;
    resetZoom();
    imageError = false;
  }

  function handleWheel(e) {
    e.preventDefault();
    const zoomFactor = -e.deltaY * 0.0025;
    const newScale = Math.min(Math.max(1, scale + zoomFactor), 6);
    if (newScale <= 1.02) {
      resetZoom();
    } else {
      scale = newScale;
    }
  }

  function handleDoubleClick(e) {
    isAnimating = true;
    if (scale > 1.1) {
      resetZoom();
    } else {
      scale = 2.5;
      translateX = 0;
      translateY = 0;
    }
    setTimeout(() => { isAnimating = false; }, 260);
  }

  // Mobile Pinch-to-Zoom & Pan Touch Events
  function handleTouchStart(e) {
    if (e.touches.length === 2) {
      isPanning = false;
      initialDistance = Math.hypot(
        e.touches[0].clientX - e.touches[1].clientX,
        e.touches[0].clientY - e.touches[1].clientY
      );
      initialScale = scale;
    } else if (e.touches.length === 1) {
      const now = Date.now();
      if (now - lastTap < 300) {
        handleDoubleClick(e.touches[0]);
        lastTap = 0;
        return;
      }
      lastTap = now;

      if (scale > 1) {
        isPanning = true;
        startX = e.touches[0].clientX - translateX;
        startY = e.touches[0].clientY - translateY;
      }
    }
  }

  function handleTouchMove(e) {
    if (e.touches.length === 2 && initialDistance > 0) {
      e.preventDefault();
      const currentDistance = Math.hypot(
        e.touches[0].clientX - e.touches[1].clientX,
        e.touches[0].clientY - e.touches[1].clientY
      );
      const newScale = Math.min(Math.max(1, initialScale * (currentDistance / initialDistance)), 6);
      scale = newScale;
      if (scale <= 1.02) {
        translateX = 0;
        translateY = 0;
      }
    } else if (e.touches.length === 1 && isPanning && scale > 1) {
      e.preventDefault();
      translateX = e.touches[0].clientX - startX;
      translateY = e.touches[0].clientY - startY;
    }
  }

  function handleTouchEnd(e) {
    if (e.touches.length < 2) {
      initialDistance = 0;
    }
    if (e.touches.length === 0) {
      isPanning = false;
      if (scale <= 1.02) {
        resetZoom();
      }
    }
  }

  // Desktop Mouse Drag Pan
  function handleMouseDown(e) {
    if (scale <= 1 || e.button !== 0) return;
    isPanning = true;
    startX = e.clientX - translateX;
    startY = e.clientY - translateY;
    e.preventDefault();
  }

  function handleMouseMove(e) {
    if (!isPanning || scale <= 1) return;
    translateX = e.clientX - startX;
    translateY = e.clientY - startY;
  }

  function handleMouseUp() {
    isPanning = false;
  }

  // Keyboard Navigation
  function handleKeyDown(e) {
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
    if (e.key === 'ArrowLeft') {
      if (currentIndex > 0) {
        const prev = screenshots[currentIndex - 1];
        onSelectShot(prev.id, prev.timestamp);
      }
    } else if (e.key === 'ArrowRight') {
      if (currentIndex >= 0 && currentIndex < screenshots.length - 1) {
        const next = screenshots[currentIndex + 1];
        onSelectShot(next.id, next.timestamp);
      }
    } else if (e.key === 'd' || e.key === 'D') {
      onDownload();
    }
  }

  onMount(() => {
    window.addEventListener('mouseup', handleMouseUp);
    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('mouseup', handleMouseUp);
      window.removeEventListener('keydown', handleKeyDown);
    };
  });
</script>

<div class="flex-1 w-full h-full min-h-0 flex flex-col bg-black overflow-hidden relative select-none">
  <!-- Central In-Place Zoomable & Pannable Viewport (No extra modals/buttons) -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="flex-1 w-full min-h-0 bg-black overflow-hidden relative flex items-center justify-center p-2 select-none"
    style="touch-action: none;"
    onwheel={handleWheel}
    onmousedown={handleMouseDown}
    onmousemove={handleMouseMove}
    ontouchstart={handleTouchStart}
    ontouchmove={handleTouchMove}
    ontouchend={handleTouchEnd}
  >
    {#if currentShot}
      {#if imageError}
        <div class="absolute inset-0 flex flex-col items-center justify-center gap-2 bg-black/80 z-20 text-zinc-400 text-xs">
          <span>Failed to load capture image</span>
          <button
            type="button"
            onclick={() => { imageError = false; }}
            class="px-3 py-1 rounded-full bg-zinc-850 hover:bg-zinc-800 text-white text-xs border border-white/10 cursor-pointer"
          >
            Retry
          </button>
        </div>
      {/if}

      {#key currentShot.id}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <img
          src={currentShot.url}
          alt={`Desktop capture ${currentShot.timestamp}`}
          draggable="false"
          onerror={() => { imageError = true; }}
          ondblclick={handleDoubleClick}
          class="max-w-full max-h-full object-contain block select-none will-change-transform {scale > 1 ? (isPanning ? 'cursor-grabbing' : 'cursor-grab') : 'cursor-zoom-in'}"
          style="transform: translate3d({translateX}px, {translateY}px, 0px) scale({scale}); {isAnimating ? 'transition: transform 0.25s cubic-bezier(0.16, 1, 0.3, 1);' : ''}"
        />
      {/key}

      <!-- Subtle Zoom Indicator (Only when zoomed in) -->
      {#if scale > 1.05}
        <button
          type="button"
          onclick={resetZoom}
          class="absolute top-3 right-3 px-2 py-1 rounded-full bg-black/70 border border-white/20 text-zinc-300 hover:text-white text-[11px] font-mono cursor-pointer backdrop-blur-md transition-all shadow-lg active:scale-95"
          title="Reset zoom to fit"
        >
          {Math.round(scale * 100)}% • Reset
        </button>
      {/if}
    {:else}
      <!-- Empty State Placeholder -->
      <div class="w-full h-full flex flex-col items-center justify-center gap-3 text-zinc-500 p-8 text-center">
        <div class="w-14 h-14 rounded-2xl bg-zinc-900 border border-zinc-800 flex items-center justify-center text-zinc-400 shadow-inner">
          <svg class="w-7 h-7" fill="none" stroke="currentColor" stroke-width="1.8" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" d="M9.75 17L9 20l-1 1h8l-1-1-.75-3M3 13h18M5 17h14a2 2 0 002-2V5a2 2 0 00-2-2H5a2 2 0 00-2 2v10a2 2 0 002 2z"></path></svg>
        </div>
        <span class="text-sm font-medium text-zinc-300">No screenshots captured yet</span>
        <span class="text-xs text-zinc-500 max-w-sm">Tap the camera button below to take a screenshot.</span>
      </div>
    {/if}
  </div>

  <!-- Bottom Horizontal Reel Filmstrip & Ergonomic Camera Shutter Dock (With Download buttons close to finger) -->
  <ViewerFilmstrip
    screenshots={screenshots}
    currentSelectedId={currentSelectedId}
    currentPass={currentPass}
    onSelect={onSelectShot}
    onCapture={onCapture}
    onClear={onClear}
    onDownload={onDownload}
    onBatchDownload={onBatchDownload}
  />
</div>
