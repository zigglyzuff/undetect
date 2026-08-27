const http = require('http');
const os = require('os');

const MAX_SESSION_SCREENSHOTS = 25;
let server = null;
let sessionScreenshots = [];
let serverConfig = {
  port: 9090,
  password: '1234',
  running: false,
};

function getLocalIpAddresses() {
  const interfaces = os.networkInterfaces();
  const addresses = [];
  for (const name of Object.keys(interfaces)) {
    for (const iface of interfaces[name]) {
      if (iface.family === 'IPv4' && !iface.internal) {
        addresses.push(iface.address);
      }
    }
  }
  return addresses.length > 0 ? addresses : ['127.0.0.1'];
}

function pushScreenshot(jpegBuffer) {
  const id = Date.now().toString();
  const timestamp = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });
  const entry = { id, timestamp, buffer: jpegBuffer };

  sessionScreenshots.push(entry);
  if (sessionScreenshots.length > MAX_SESSION_SCREENSHOTS) {
    sessionScreenshots.shift();
  }
  console.log(`[UndetectServer] New screenshot added (${Math.round(jpegBuffer.length / 1024)} KB). Total: ${sessionScreenshots.length}`);
  return { success: true, id, timestamp };
}

function clearSession() {
  sessionScreenshots = [];
  console.log('[UndetectServer] Session screenshots cleared.');
}

function startServer(port = 9090, password = '1234', onRequestCapture = null) {
  stopServer();

  serverConfig.port = parseInt(port, 10) || 9090;
  serverConfig.password = password || '';

  server = http.createServer((req, res) => {
    const parsedUrl = new URL(req.url, `http://${req.headers.host || 'localhost'}`);
    const pathname = parsedUrl.pathname;
    const reqPass = parsedUrl.searchParams.get('pass') || parsedUrl.searchParams.get('pin');

    res.setHeader('Access-Control-Allow-Origin', '*');
    res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS');
    res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

    if (req.method === 'OPTIONS') {
      res.writeHead(204);
      res.end();
      return;
    }

    if (pathname === '/' || pathname === '/index.html') {
      if (serverConfig.password && reqPass !== serverConfig.password) {
        res.writeHead(200, { 'Content-Type': 'text/html' });
        res.end(renderPasswordPage(reqPass ? 'Invalid Password' : ''));
        return;
      }
      res.writeHead(200, { 'Content-Type': 'text/html' });
      res.end(renderDashboardPage(serverConfig.password));
      return;
    }

    if (serverConfig.password && reqPass !== serverConfig.password) {
      res.writeHead(403, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'Unauthorized' }));
      return;
    }

    if (pathname === '/api/session') {
      const list = sessionScreenshots.map(s => ({ id: s.id, timestamp: s.timestamp }));
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ screenshots: list }));
      return;
    }

    if (pathname === '/api/screenshot') {
      const id = parsedUrl.searchParams.get('id');
      const target = id ? sessionScreenshots.find(s => s.id === id) : sessionScreenshots[sessionScreenshots.length - 1];
      if (target && target.buffer) {
        res.writeHead(200, {
          'Content-Type': 'image/jpeg',
          'Content-Length': target.buffer.length,
          'Cache-Control': 'no-cache',
        });
        res.end(target.buffer);
      } else {
        res.writeHead(404, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'No screenshot found' }));
      }
      return;
    }

    if (pathname === '/api/capture' && req.method === 'POST') {
      if (onRequestCapture) {
        onRequestCapture();
      }
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ success: true, message: 'Capture triggered' }));
      return;
    }

    if (pathname === '/api/session/clear' && req.method === 'POST') {
      clearSession();
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ success: true }));
      return;
    }

    res.writeHead(404, { 'Content-Type': 'text/plain' });
    res.end('Not Found');
  });

  server.listen(serverConfig.port, '0.0.0.0', () => {
    serverConfig.running = true;
    console.log(`[UndetectServer] Remote Server running at http://0.0.0.0:${serverConfig.port}`);
  });
}

function stopServer() {
  if (server) {
    try { server.close(); } catch (e) {}
    server = null;
  }
  serverConfig.running = false;
}

function getStatus() {
  const ips = getLocalIpAddresses();
  const localUrls = ips.map(ip => `http://${ip}:${serverConfig.port}/?pass=${encodeURIComponent(serverConfig.password)}`);

  return {
    running: serverConfig.running,
    port: serverConfig.port,
    password: serverConfig.password,
    localUrls,
    screenshotCount: sessionScreenshots.length,
  };
}

function renderPasswordPage(errorMsg = '') {
  return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Undetect Remote Access</title>
    <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@400;500;600;700&display=swap" rel="stylesheet">
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; font-family: 'Outfit', sans-serif; }
        body { background: #08090d; color: #f1f5f9; min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 20px; }
        .card { background: rgba(30, 41, 59, 0.4); border: 1px solid rgba(255, 255, 255, 0.08); border-radius: 16px; width: 100%; max-width: 360px; padding: 28px; text-align: center; }
        h2 { font-size: 1.4rem; margin-bottom: 6px; }
        p { color: #94a3b8; font-size: 0.88rem; margin-bottom: 20px; }
        input { width: 100%; padding: 12px 14px; background: rgba(15, 23, 42, 0.8); border: 1px solid rgba(255, 255, 255, 0.15); border-radius: 8px; color: #fff; font-size: 1.1rem; text-align: center; margin-bottom: 16px; outline: none; }
        button { width: 100%; padding: 12px; background: #6366f1; border: none; border-radius: 8px; color: #fff; font-weight: 600; font-size: 0.95rem; cursor: pointer; }
        .error { color: #ef4444; font-size: 0.85rem; margin-bottom: 14px; }
    </style>
</head>
<body>
    <div class="card">
        <h2>Undetect Remote Access</h2>
        <p>Enter password to unlock viewer</p>
        ${errorMsg ? `<div class="error">${errorMsg}</div>` : ''}
        <form method="GET" action="/">
            <input type="password" name="pass" placeholder="••••••••" autofocus required>
            <button type="submit">Unlock Dashboard</button>
        </form>
    </div>
</body>
</html>`;
}

function renderDashboardPage(password) {
  return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=3.0, user-scalable=yes">
    <title>Undetect Remote Viewer</title>
    <link href="https://fonts.googleapis.com/css2?family=Outfit:wght@400;500;600;700&display=swap" rel="stylesheet">
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; font-family: 'Outfit', sans-serif; }
        ::-webkit-scrollbar { display: none; }
        body { background: #08090d; color: #f1f5f9; min-height: 100vh; display: flex; flex-direction: column; scrollbar-width: none; }
        header { background: rgba(15, 23, 42, 0.9); border-bottom: 1px solid rgba(255, 255, 255, 0.1); padding: 12px 16px; display: flex; align-items: center; justify-content: space-between; position: sticky; top: 0; z-index: 100; gap: 8px; }
        .logo { font-weight: 700; font-size: 1.05rem; display: flex; align-items: center; gap: 8px; }
        .dot { width: 8px; height: 8px; background: #22c55e; border-radius: 50%; box-shadow: 0 0 8px #22c55e; }
        .btn { padding: 8px 12px; border-radius: 8px; border: 1px solid rgba(255,255,255,0.12); background: rgba(255,255,255,0.06); color: #fff; font-size: 0.82rem; font-weight: 500; cursor: pointer; transition: 0.2s; display: inline-flex; align-items: center; gap: 6px; justify-content: center; }
        .btn:hover { background: rgba(255,255,255,0.12); }
        .btn-primary { background: #6366f1; border-color: #6366f1; }
        .btn-danger { background: rgba(239, 68, 68, 0.2); border-color: rgba(239, 68, 68, 0.4); color: #ef4444; }

        .icon-svg { width: 14px; height: 14px; stroke: currentColor; fill: none; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round; }

        .main-container { flex: 1; display: grid; grid-template-columns: 280px 1fr; gap: 16px; padding: 16px; max-width: 1400px; margin: 0 auto; width: 100%; }
        @media (max-width: 768px) {
            .main-container { grid-template-columns: 1fr; padding: 10px; gap: 10px; }
            .sidebar { max-height: 140px; }
            .shots-list { flex-direction: row !important; overflow-x: auto; overflow-y: hidden; }
            .shot-item { white-space: nowrap; flex-shrink: 0; }
        }
        
        .sidebar { background: rgba(30, 41, 59, 0.35); border: 1px solid rgba(255, 255, 255, 0.08); border-radius: 14px; padding: 14px; display: flex; flex-direction: column; gap: 10px; }
        .shots-list { display: flex; flex-direction: column; gap: 6px; overflow-y: auto; max-height: 480px; }
        .shot-item { padding: 10px 12px; background: rgba(255,255,255,0.03); border: 1px solid rgba(255,255,255,0.05); border-radius: 8px; cursor: pointer; font-size: 0.82rem; transition: 0.2s; display: flex; align-items: center; gap: 6px; }
        .shot-item:hover, .shot-item.active { background: rgba(99, 102, 241, 0.25); border-color: #6366f1; font-weight: 600; }
        
        .preview-card { background: rgba(30, 41, 59, 0.35); border: 1px solid rgba(255, 255, 255, 0.08); border-radius: 14px; padding: 14px; display: flex; flex-direction: column; gap: 12px; }
        .preview-actions { display: flex; align-items: center; justify-content: space-between; flex-wrap: wrap; gap: 8px; }
        .img-container { background: #000; border-radius: 10px; overflow: hidden; display: flex; align-items: center; justify-content: center; min-height: 300px; border: 1px solid rgba(255,255,255,0.08); cursor: zoom-in; }
        .img-container img { max-width: 100%; height: auto; object-fit: contain; width: 100%; }

        .modal { display: none; position: fixed; inset: 0; background: rgba(0,0,0,0.95); z-index: 2000; flex-direction: column; align-items: center; justify-content: center; padding: 10px; }
        .modal.open { display: flex; }
        .modal img { max-width: 100%; max-height: 90vh; object-fit: contain; border-radius: 6px; }
        .modal-close { position: absolute; top: 16px; right: 16px; background: rgba(255,255,255,0.2); border: none; color: #fff; width: 36px; height: 36px; border-radius: 50%; font-size: 1.2rem; cursor: pointer; }

        .toast { position: fixed; bottom: 16px; right: 16px; background: #10b981; color: #fff; padding: 10px 14px; border-radius: 8px; font-size: 0.85rem; opacity: 0; transition: 0.3s; pointer-events: none; z-index: 3000; }
        .toast.show { opacity: 1; }
    </style>
</head>
<body>
    <header>
        <div class="logo"><div class="dot"></div> Undetect Remote Viewer</div>
        <div style="display:flex; gap:8px;">
            <button class="btn btn-danger" onclick="clearAll()">
                <svg class="icon-svg" viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>
                Clear
            </button>
            <button class="btn btn-primary" onclick="triggerCapture()">
                <svg class="icon-svg" viewBox="0 0 24 24"><path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z"></path><circle cx="12" cy="13" r="4"></circle></svg>
                Capture Screen
            </button>
        </div>
    </header>

    <div class="main-container">
        <div class="sidebar">
            <div style="font-weight:600; font-size:0.9rem; margin-bottom:4px;">Session Shots (<span id="shotCount">0</span>)</div>
            <div class="shots-list" id="shotsList">
                <div style="padding:16px; color:#64748b; font-size:0.85rem;">No screenshots yet.</div>
            </div>
        </div>

        <div class="preview-card">
            <div class="preview-actions">
                <span id="activeTimestamp" style="font-size:0.85rem; color:#94a3b8; font-weight:500;">Select a screenshot</span>
                <div style="display:flex; gap:8px;">
                    <button class="btn" onclick="copyActiveImage()">
                        <svg class="icon-svg" viewBox="0 0 24 24"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path></svg>
                        Copy Image
                    </button>
                    <a id="downloadLink" href="#" download="screenshot.jpg" class="btn btn-primary" style="text-decoration:none;">
                        <svg class="icon-svg" viewBox="0 0 24 24"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
                        Download
                    </a>
                </div>
            </div>
            <div class="img-container" onclick="openModal()">
                <img id="previewImage" src="" style="display:none;">
                <div id="noImgText" style="color:#64748b; padding:40px; font-size:0.9rem;">No screenshot loaded.<br>Tap image to view fullscreen zoom.</div>
            </div>
        </div>
    </div>

    <!-- Fullscreen Modal -->
    <div id="zoomModal" class="modal" onclick="closeModal()">
        <button class="modal-close" onclick="closeModal()">✕</button>
        <img id="modalImage" src="" alt="Fullscreen Screenshot">
    </div>

    <div id="toast" class="toast">Action completed</div>

    <script>
        const PASS = "${password}";
        let selectedId = null;

        function showToast(msg) {
            const toast = document.getElementById('toast');
            toast.innerText = msg;
            toast.classList.add('show');
            setTimeout(() => toast.classList.remove('show'), 2000);
        }

        function openModal() {
            const img = document.getElementById('previewImage');
            if (img.src && img.style.display !== 'none') {
                document.getElementById('modalImage').src = img.src;
                document.getElementById('zoomModal').classList.add('open');
            }
        }

        function closeModal() {
            document.getElementById('zoomModal').classList.remove('open');
        }

        async function fetchSession() {
            try {
                const res = await fetch('/api/session?pass=' + encodeURIComponent(PASS));
                const data = await res.json();
                const list = data.screenshots || [];
                document.getElementById('shotCount').innerText = list.length;
                const container = document.getElementById('shotsList');
                if (list.length === 0) {
                    container.innerHTML = '<div style="padding:16px; color:#64748b; font-size:0.85rem;">No screenshots yet.</div>';
                    document.getElementById('previewImage').style.display = 'none';
                    document.getElementById('noImgText').style.display = 'block';
                    selectedId = null;
                    return;
                }
                const cameraSvg = '<svg class="icon-svg" viewBox="0 0 24 24"><path d="M23 19a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4l2-3h6l2 3h4a2 2 0 0 1 2 2z"></path><circle cx="12" cy="13" r="4"></circle></svg>';
                container.innerHTML = list.slice().reverse().map(item => \`
                    <div class="shot-item \${item.id === selectedId ? 'active' : ''}" onclick="selectShot('\${item.id}', '\${item.timestamp}')">
                        \${cameraSvg} \${item.timestamp}
                    </div>
                \`).join('');
                if (!selectedId || !list.some(s => s.id === selectedId)) {
                    selectShot(list[list.length - 1].id, list[list.length - 1].timestamp);
                }
            } catch (e) {}
        }

        function selectShot(id, timestamp) {
            selectedId = id;
            var cleanTime = timestamp ? timestamp.replace(/[: ]/g, '-') : Date.now();
            var dateStr = new Date().toISOString().split('T')[0];
            var fileName = 'Undetect_' + dateStr + '_' + cleanTime + '.jpg';

            if (timestamp) {
                document.getElementById('activeTimestamp').innerText = 'Captured at ' + timestamp;
            }
            const img = document.getElementById('previewImage');
            const imgUrl = '/api/screenshot?id=' + id + '&pass=' + encodeURIComponent(PASS) + '&t=' + Date.now();
            img.src = imgUrl;
            img.style.display = 'block';
            document.getElementById('noImgText').style.display = 'none';
            
            const dlLink = document.getElementById('downloadLink');
            dlLink.href = imgUrl;
            dlLink.download = fileName;

            const items = document.querySelectorAll('.shot-item');
            items.forEach(el => el.classList.remove('active'));
            fetchSession();
        }

        async function triggerCapture() {
            showToast('Capturing screen...');
            await fetch('/api/capture?pass=' + encodeURIComponent(PASS), { method: 'POST' });
            setTimeout(fetchSession, 600);
        }

        async function clearAll() {
            if (!confirm('Clear all session screenshots?')) return;
            await fetch('/api/session/clear?pass=' + encodeURIComponent(PASS), { method: 'POST' });
            showToast('Session cleared');
            fetchSession();
        }

        async function copyActiveImage() {
            if (!selectedId) return showToast('No screenshot selected');
            const imgUrl = '/api/screenshot?id=' + selectedId + '&pass=' + encodeURIComponent(PASS);
            try {
                const response = await fetch(imgUrl);
                const blob = await response.blob();
                const img = new Image();
                img.src = URL.createObjectURL(blob);
                await new Promise(resolve => img.onload = resolve);
                const canvas = document.createElement('canvas');
                canvas.width = img.width;
                canvas.height = img.height;
                const ctx = canvas.getContext('2d');
                ctx.drawImage(img, 0, 0);
                canvas.toBlob(async (pngBlob) => {
                    try {
                        await navigator.clipboard.write([new ClipboardItem({ 'image/png': pngBlob })]);
                        showToast('Image copied to clipboard!');
                    } catch (err) {
                        navigator.clipboard.writeText(window.location.origin + imgUrl);
                        showToast('Link copied to clipboard!');
                    }
                }, 'image/png');
            } catch (err) {
                showToast('Failed to copy image');
            }
        }

        setInterval(fetchSession, 2000);
        fetchSession();
    </script>
</body>
</html>`;
}

module.exports = {
  startServer,
  stopServer,
  pushScreenshot,
  clearSession,
  getStatus,
};
