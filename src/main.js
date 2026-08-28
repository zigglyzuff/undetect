if (require('electron-squirrel-startup')) {
  process.exit(0);
}

const { app, BrowserWindow, ipcMain, globalShortcut, desktopCapturer, screen, shell } = require('electron');
const path = require('path');
const https = require('https');
const fs = require('fs');
const { spawn } = require('child_process');
const QRCode = require('qrcode');
const { updateElectronApp } = require('update-electron-app');
const server = require('./server');

let mainWindow = null;
let skipTaskbarState = false;
let contentProtectedState = true;
const GITHUB_REPO_SLUG = 'zigglyzuff/undetect';

// Standardized Electron Auto-Updater from GitHub Releases
try {
  updateElectronApp({
    repo: GITHUB_REPO_SLUG,
    updateInterval: '5 minutes',
  });
} catch (e) {
  console.log('[UndetectMain] update-electron-app setup note:', e.message);
}

function createWindow() {
  mainWindow = new BrowserWindow({
    width: 460,
    height: 680,
    resizable: false,
    frame: false,
    transparent: true,
    backgroundColor: '#00000000',
    icon: path.join(__dirname, 'assets/logo.png'),
    title: 'Undetect Screen Server',
    webPreferences: {
      nodeIntegration: true,
      contextIsolation: false,
      backgroundThrottling: false,
    },
  });

  mainWindow.loadFile(path.join(__dirname, 'index.html'));

  mainWindow.setContentProtection(contentProtectedState);

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

async function captureScreen() {
  try {
    const primaryDisplay = screen.getPrimaryDisplay();
    const { width, height } = primaryDisplay.size;

    const maxWidth = 1920;
    let destW = width;
    let destH = height;
    if (width > maxWidth) {
      destW = maxWidth;
      destH = Math.round(height * (maxWidth / width));
    }

    const sources = await desktopCapturer.getSources({
      types: ['screen'],
      thumbnailSize: { width: destW, height: destH },
    });

    if (sources.length > 0) {
      const jpegBuffer = sources[0].thumbnail.toJPEG(85);
      server.pushScreenshot(jpegBuffer);
      if (mainWindow && !mainWindow.isDestroyed()) {
        mainWindow.webContents.send('screenshot-captured', { success: true });
      }
      return { success: true };
    }
    return { success: false, error: 'No display source found' };
  } catch (err) {
    console.error('[UndetectMain] Screen capture failed:', err.message);
    return { success: false, error: err.message };
  }
}

function checkForGitHubUpdates() {
  return new Promise((resolve) => {
    const options = {
      hostname: 'api.github.com',
      path: `/repos/${GITHUB_REPO_SLUG}/releases/latest`,
      headers: { 'User-Agent': 'Undetect-App' },
    };

    https.get(options, (res) => {
      let body = '';
      res.on('data', chunk => body += chunk);
      res.on('end', () => {
        try {
          const release = JSON.parse(body);
          if (release && release.tag_name) {
            const latestVer = release.tag_name.replace(/^v/, '');
            const currentVer = app.getVersion();
            if (latestVer !== currentVer) {
              const exeAsset = release.assets ? release.assets.find(a => a.name.endsWith('.exe')) : null;
              const downloadUrl = exeAsset ? exeAsset.browser_download_url : (release.assets && release.assets[0] ? release.assets[0].browser_download_url : release.html_url);

              return resolve({
                updateAvailable: true,
                version: latestVer,
                currentVersion: currentVer,
                downloadUrl: downloadUrl,
                htmlUrl: release.html_url,
                notes: release.body || '',
              });
            }
          }
          resolve({ updateAvailable: false, currentVersion: app.getVersion() });
        } catch (e) {
          resolve({ updateAvailable: false, error: e.message });
        }
      });
    }).on('error', (err) => resolve({ updateAvailable: false, error: err.message }));
  });
}

function downloadAndInstallUpdate(downloadUrl) {
  return new Promise((resolve, reject) => {
    if (!downloadUrl) return reject(new Error('No download URL provided'));

    const tempDir = app.getPath('temp');
    const installerPath = path.join(tempDir, `Undetect-Update-Setup.exe`);

    const file = fs.createWriteStream(installerPath);

    function fetchUrl(url) {
      https.get(url, { headers: { 'User-Agent': 'Undetect-App' } }, (res) => {
        if (res.statusCode === 301 || res.statusCode === 302) {
          return fetchUrl(res.headers.location);
        }
        if (res.statusCode !== 200) {
          return reject(new Error(`Failed to download update (${res.statusCode})`));
        }

        res.pipe(file);
        file.on('finish', () => {
          file.close(() => {
            console.log(`[UndetectMain] Download completed: ${installerPath}`);
            try {
              spawn(installerPath, [], { detached: true, stdio: 'ignore' }).unref();
              app.quit();
              resolve({ success: true });
            } catch (err) {
              reject(err);
            }
          });
        });
      }).on('error', err => reject(err));
    }

    fetchUrl(downloadUrl);
  });
}

app.whenReady().then(() => {
  createWindow();

  const isMac = process.platform === 'darwin';
  const shortcut = isMac ? 'Cmd+Alt+S' : 'Ctrl+Alt+S';

  try {
    globalShortcut.register(shortcut, () => {
      console.log(`[UndetectMain] Shortcut ${shortcut} triggered`);
      captureScreen();
    });
  } catch (err) {
    console.error('[UndetectMain] Shortcut registration error:', err);
  }

  server.startServer(9090, '1234', () => captureScreen());

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('will-quit', () => {
  globalShortcut.unregisterAll();
  server.stopServer();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') app.quit();
});

// IPC Handlers
ipcMain.handle('start-server', (event, { port, password }) => {
  server.startServer(port, password, () => captureScreen());
  return server.getStatus();
});

ipcMain.handle('stop-server', () => {
  server.stopServer();
  return server.getStatus();
});

ipcMain.handle('get-status', () => {
  return {
    ...server.getStatus(),
    skipTaskbar: skipTaskbarState,
    contentProtected: contentProtectedState,
    appVersion: app.getVersion(),
    gitHubRepo: GITHUB_REPO_SLUG,
  };
});

ipcMain.handle('generate-qr', async (event, url) => {
  try {
    if (!url) return { success: false };
    const qrDataUrl = await QRCode.toDataURL(url, {
      margin: 1,
      width: 180,
      color: { dark: '#ffffff', light: '#00000000' },
    });
    return { success: true, qrDataUrl };
  } catch (e) {
    return { success: false, error: e.message };
  }
});

ipcMain.handle('capture-now', async () => {
  return await captureScreen();
});

ipcMain.handle('clear-session', () => {
  server.clearSession();
  return { success: true };
});

ipcMain.handle('toggle-taskbar', (event, hide) => {
  skipTaskbarState = hide;
  if (mainWindow && !mainWindow.isDestroyed()) {
    try {
      mainWindow.setSkipTaskbar(skipTaskbarState);
    } catch (e) {}
  }
  return { success: true, skipTaskbar: skipTaskbarState };
});

ipcMain.handle('toggle-content-protection', (event, protect) => {
  contentProtectedState = protect;
  if (mainWindow && !mainWindow.isDestroyed()) {
    try {
      mainWindow.setContentProtection(contentProtectedState);
    } catch (e) {}
  }
  return { success: true, contentProtected: contentProtectedState };
});

ipcMain.handle('check-updates', async () => {
  return await checkForGitHubUpdates();
});

ipcMain.handle('download-install-update', async (event, downloadUrl) => {
  try {
    return await downloadAndInstallUpdate(downloadUrl);
  } catch (err) {
    return { success: false, error: err.message };
  }
});

ipcMain.handle('open-external-url', (event, url) => {
  if (url) shell.openExternal(url);
});

ipcMain.handle('window-minimize', () => {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.minimize();
  }
});

ipcMain.handle('window-close', () => {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.destroy();
  }
  app.quit();
});
