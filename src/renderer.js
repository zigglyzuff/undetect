const { ipcRenderer } = require('electron');

let latestDownloadUrl = '';

function showToast(msg) {
  const toast = document.getElementById('toast');
  toast.innerText = msg;
  toast.classList.add('show');
  setTimeout(() => toast.classList.remove('show'), 2000);
}

function copyText(text) {
  navigator.clipboard.writeText(text);
  showToast('URL Copied to Clipboard!');
}

async function showQrModal(url) {
  const res = await ipcRenderer.invoke('generate-qr', url);
  if (res.success && res.qrDataUrl) {
    document.getElementById('qrImage').src = res.qrDataUrl;
    document.getElementById('qrModal').classList.add('open');
  } else {
    showToast('Failed to generate QR Code');
  }
}

function closeQrModal() {
  document.getElementById('qrModal').classList.remove('open');
}

async function refreshStatus() {
  try {
    const status = await ipcRenderer.invoke('get-status');
    const badge = document.getElementById('statusBadge');
    
    if (status.running) {
      badge.innerHTML = '<span class="dot-indicator"></span> Active';
      badge.className = 'status-badge active';
    } else {
      badge.innerHTML = '<span class="dot-indicator"></span> Stopped';
      badge.className = 'status-badge inactive';
    }

    if (status.appVersion) {
      document.getElementById('verTag').innerText = 'v' + status.appVersion;
    }

    document.getElementById('shotCount').innerText = status.screenshotCount || 0;
    document.getElementById('contentProtectToggle').checked = !!status.contentProtected;
    document.getElementById('taskbarToggle').checked = !!status.skipTaskbar;

    const urlsList = document.getElementById('urlsList');
    if (status.localUrls && status.localUrls.length > 0) {
      const copyIcon = '<svg class="icon-svg" viewBox="0 0 24 24"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>';
      const qrIcon = '<svg class="icon-svg" viewBox="0 0 24 24"><rect x="3" y="3" width="7" height="7"></rect><rect x="14" y="3" width="7" height="7"></rect><rect x="14" y="14" width="7" height="7"></rect><rect x="3" y="14" width="7" height="7"></rect></svg>';
      
      urlsList.innerHTML = status.localUrls.map(url => `
        <div class="url-item">
          <span class="url-text">${url}</span>
          <div style="display:flex; gap:4px;">
            <button class="action-btn" onclick="showQrModal('${url}')">${qrIcon} QR</button>
            <button class="action-btn" onclick="copyText('${url}')">${copyIcon} Copy</button>
          </div>
        </div>
      `).join('');
    } else {
      urlsList.innerHTML = '<div style="font-size:0.8rem; color:#64748b;">Server stopped</div>';
    }
  } catch (err) {
    console.error(err);
  }
}

async function checkUpdates(manual = false) {
  showToast('Checking GitHub for updates...');
  const res = await ipcRenderer.invoke('check-updates');

  if (res.updateAvailable) {
    latestDownloadUrl = res.downloadUrl || res.htmlUrl;
    document.getElementById('updateVerText').innerText = 'v' + res.version;
    document.getElementById('updateBanner').classList.add('show');
    showToast(`New Version v${res.version} Available!`);
  } else if (manual) {
    showToast(`Latest version (v${res.currentVersion || '1.0.0'}) is installed`);
  }
}

async function downloadUpdate() {
  if (!latestDownloadUrl) {
    return showToast('No update package URL found');
  }

  showToast('Downloading update package...');
  const res = await ipcRenderer.invoke('download-install-update', latestDownloadUrl);

  if (res && res.error) {
    showToast('Download failed: ' + res.error);
  }
}

async function updateServer() {
  const port = document.getElementById('portInput').value || '9090';
  const password = document.getElementById('passwordInput').value || '1234';
  
  await ipcRenderer.invoke('start-server', { port, password });
  showToast('Server Updated & Started!');
  refreshStatus();
}

async function stopServer() {
  await ipcRenderer.invoke('stop-server');
  showToast('Server Stopped');
  refreshStatus();
}

async function triggerCapture() {
  showToast('Capturing screenshot...');
  const res = await ipcRenderer.invoke('capture-now');
  if (res.success) {
    showToast('Screenshot Captured!');
  } else {
    showToast('Capture Failed: ' + (res.error || 'Unknown'));
  }
  refreshStatus();
}

async function clearSession() {
  await ipcRenderer.invoke('clear-session');
  showToast('Session Cleared');
  refreshStatus();
}

async function toggleTaskbar() {
  const checkbox = document.getElementById('taskbarToggle');
  const target = !checkbox.checked;
  const res = await ipcRenderer.invoke('toggle-taskbar', target);
  checkbox.checked = res.skipTaskbar;
  showToast(target ? 'Hidden from Taskbar' : 'Shown in Taskbar');
}

async function toggleContentProtection() {
  const checkbox = document.getElementById('contentProtectToggle');
  const target = !checkbox.checked;
  const res = await ipcRenderer.invoke('toggle-content-protection', target);
  checkbox.checked = res.contentProtected;
  showToast(target ? 'App Window Protected from Capture' : 'App Window Protection Off');
}

function minimizeWindow() {
  ipcRenderer.invoke('window-minimize');
}

function closeWindow() {
  ipcRenderer.invoke('window-close');
}

ipcRenderer.on('screenshot-captured', () => {
  showToast('Shortcut Captured Screenshot!');
  refreshStatus();
});

// Initial load
refreshStatus();
setInterval(refreshStatus, 3000);
