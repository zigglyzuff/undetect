import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import zlib from 'node:zlib';

const DEFAULT_REPO_SLUG = 'zigglyzuff/undetect';
const DEFAULT_RELEASE_TAG = 'ui-latest';

// Standard IEEE 802.3 CRC-32 lookup table for deterministic cross-platform ZIP archives.
const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let i = 0; i < 256; i++) {
    let c = i;
    for (let k = 0; k < 8; k++) {
      c = (c & 1) !== 0 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    table[i] = c >>> 0;
  }
  return table;
})();

function crc32(buf) {
  let crc = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    crc = CRC_TABLE[(crc ^ buf[i]) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

/**
 * Create a standard ZIP buffer from `{ name, data }[]` using forward-slash paths.
 */
export function createZipBuffer(entries) {
  const localParts = [];
  const centralParts = [];
  let offset = 0;

  for (const entry of entries) {
    const cleanName = entry.name.replace(/\\/g, '/').replace(/^\/+/, '');
    const nameBuf = Buffer.from(cleanName, 'utf8');
    const rawData = Buffer.isBuffer(entry.data) ? entry.data : Buffer.from(entry.data);
    const compressed = zlib.deflateRawSync(rawData, { level: 9 });
    const useDeflate = compressed.length < rawData.length;
    const payload = useDeflate ? compressed : rawData;
    const method = useDeflate ? 8 : 0;
    const sum = crc32(rawData);

    const localHeader = Buffer.alloc(30);
    localHeader.writeUInt32LE(0x04034b50, 0);
    localHeader.writeUInt16LE(20, 4); // version needed
    localHeader.writeUInt16LE(0x0800, 6); // UTF-8 filename flag
    localHeader.writeUInt16LE(method, 8);
    localHeader.writeUInt16LE(0, 10); // mod time
    localHeader.writeUInt16LE(0x5421, 12); // mod date
    localHeader.writeUInt32LE(sum, 14);
    localHeader.writeUInt32LE(payload.length, 18);
    localHeader.writeUInt32LE(rawData.length, 22);
    localHeader.writeUInt16LE(nameBuf.length, 26);
    localHeader.writeUInt16LE(0, 28);

    localParts.push(localHeader, nameBuf, payload);

    const centralHeader = Buffer.alloc(46);
    centralHeader.writeUInt32LE(0x02014b50, 0);
    centralHeader.writeUInt16LE(20, 4);
    centralHeader.writeUInt16LE(20, 6);
    centralHeader.writeUInt16LE(0x0800, 8);
    centralHeader.writeUInt16LE(method, 10);
    centralHeader.writeUInt16LE(0, 12);
    centralHeader.writeUInt16LE(0x5421, 14);
    centralHeader.writeUInt32LE(sum, 16);
    centralHeader.writeUInt32LE(payload.length, 20);
    centralHeader.writeUInt32LE(rawData.length, 24);
    centralHeader.writeUInt16LE(nameBuf.length, 28);
    centralHeader.writeUInt16LE(0, 30);
    centralHeader.writeUInt16LE(0, 32);
    centralHeader.writeUInt16LE(0, 34);
    centralHeader.writeUInt16LE(0, 36);
    centralHeader.writeUInt32LE(0, 38);
    centralHeader.writeUInt32LE(offset, 42);

    centralParts.push(centralHeader, nameBuf);
    offset += localHeader.length + nameBuf.length + payload.length;
  }

  const centralDir = Buffer.concat(centralParts);
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(0, 4);
  eocd.writeUInt16LE(0, 6);
  eocd.writeUInt16LE(entries.length, 8);
  eocd.writeUInt16LE(entries.length, 10);
  eocd.writeUInt32LE(centralDir.length, 12);
  eocd.writeUInt32LE(offset, 16);
  eocd.writeUInt16LE(0, 20);

  return Buffer.concat([...localParts, centralDir, eocd]);
}

function collectFiles(dir, baseDir = dir) {
  const results = [];
  for (const item of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, item.name);
    if (item.isDirectory()) {
      results.push(...collectFiles(full, baseDir));
    } else if (item.isFile()) {
      const rel = path.relative(baseDir, full).replace(/\\/g, '/');
      results.push({ name: rel, data: fs.readFileSync(full) });
    }
  }
  results.sort((a, b) => a.name.localeCompare(b.name));
  return results;
}

export function canonicalManifestMessage({
  ui_version,
  min_app_version,
  max_app_version,
  sha256,
  url,
}) {
  return `${ui_version}|${min_app_version}|${max_app_version}|${sha256}|${url}`;
}

export function extractRawEd25519PublicKey(publicKeyObj) {
  const spki = publicKeyObj.export({ type: 'spki', format: 'der' });
  // Ed25519 SPKI DER is always a 12-byte ASN.1 header + 32-byte raw public key.
  if (spki.length !== 44) {
    throw new Error(`Unexpected Ed25519 SPKI DER length: ${spki.length}`);
  }
  return spki.subarray(12);
}

export function signManifest(fields, privateKeyPem) {
  const msg = canonicalManifestMessage(fields);
  const key = crypto.createPrivateKey(privateKeyPem);
  const sig = crypto.sign(null, Buffer.from(msg, 'utf8'), key);
  return {
    ...fields,
    signature: sig.toString('base64'),
  };
}

export function verifyManifestSignature(manifest, rawPubKeyBytes) {
  const spkiHeader = Buffer.from('302a300506032b6570032100', 'hex');
  const spki = Buffer.concat([spkiHeader, Buffer.from(rawPubKeyBytes)]);
  const pubKey = crypto.createPublicKey({ key: spki, format: 'der', type: 'spki' });
  const msg = canonicalManifestMessage(manifest);
  return crypto.verify(
    null,
    Buffer.from(msg, 'utf8'),
    pubKey,
    Buffer.from(manifest.signature, 'base64'),
  );
}

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i++) {
    const token = argv[i];
    if (!token.startsWith('--')) continue;
    const key = token.slice(2);
    const next = argv[i + 1];
    if (!next || next.startsWith('--')) {
      args[key] = true;
    } else {
      args[key] = next;
      i++;
    }
  }
  return args;
}

function formatRustPubKeyBytes(raw32) {
  const hexBytes = Array.from(raw32).map((b) => `0x${b.toString(16).padStart(2, '0')}`);
  const lines = [];
  for (let i = 0; i < hexBytes.length; i += 8) {
    lines.push(`    ${hexBytes.slice(i, i + 8).join(', ')},`);
  }
  return `[\n${lines.join('\n')}\n]`;
}

function runKeygen(outPath) {
  const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
  const rawPub = extractRawEd25519PublicKey(publicKey);
  const privPem = privateKey.export({ type: 'pkcs8', format: 'pem' });
  const privBase64 = Buffer.from(privPem, 'utf8').toString('base64');

  if (outPath) {
    fs.mkdirSync(path.dirname(outPath), { recursive: true });
    fs.writeFileSync(outPath, privPem, { mode: 0o600 });
    fs.writeFileSync(`${outPath}.pub.hex`, rawPub.toString('hex'));
    fs.writeFileSync(`${outPath}.pub.b64`, rawPub.toString('base64'));
  }

  console.log('Generated Ed25519 UI patch keypair:');
  console.log('Raw public key (base64):', rawPub.toString('base64'));
  console.log('Rust pubkey constant:\n' + formatRustPubKeyBytes(rawPub));
  if (outPath) {
    console.log('Private PEM written to:', outPath);
    console.log('Base64-encoded PEM length:', privBase64.length);
  }
}

function runSelfTest() {
  const { publicKey, privateKey } = crypto.generateKeyPairSync('ed25519');
  const rawPub = extractRawEd25519PublicKey(publicKey);
  const privPem = privateKey.export({ type: 'pkcs8', format: 'pem' });

  const zipBuf = createZipBuffer([
    { name: 'index.html', data: '<!doctype html><html><body>ok</body></html>' },
    { name: 'assets/main.js', data: 'console.log("ok");' },
  ]);
  if (zipBuf.readUInt32LE(0) !== 0x04034b50) {
    throw new Error('Invalid ZIP local file signature');
  }
  const sha256 = crypto.createHash('sha256').update(zipBuf).digest('hex');
  const signed = signManifest(
    {
      ui_version: '2.2.0-ui.1',
      min_app_version: '2.2.0',
      max_app_version: '2.2.0',
      sha256,
      url: `https://github.com/${DEFAULT_REPO_SLUG}/releases/download/${DEFAULT_RELEASE_TAG}/dist.zip`,
      pub_date: new Date().toISOString(),
    },
    privPem,
  );
  if (!verifyManifestSignature(signed, rawPub)) {
    throw new Error('Self-test Ed25519 manifest verification failed');
  }
  const tampered = { ...signed, sha256: '0'.repeat(64) };
  if (verifyManifestSignature(tampered, rawPub)) {
    throw new Error('Tampered manifest unexpectedly passed verification');
  }
  console.log('UI patch packaging & Ed25519 signature self-test passed.');
}

function normalizePemInput(raw) {
  const trimmed = String(raw || '').trim();
  if (!trimmed) return '';
  if (trimmed.includes('BEGIN PRIVATE KEY')) return trimmed;
  // Support base64-encoded PEM stored in CI secrets cleanly.
  return Buffer.from(trimmed, 'base64').toString('utf8');
}

function runPackage(args) {
  const rootDir = process.cwd();
  const distDir = path.resolve(rootDir, args['dist-dir'] || 'dist');
  const outDir = path.resolve(rootDir, args['out-dir'] || 'release_build/ui-patch');

  if (!fs.existsSync(path.join(distDir, 'index.html'))) {
    throw new Error(`Missing compiled dist/index.html at ${distDir}. Run "npm run build" first.`);
  }

  const pkg = JSON.parse(fs.readFileSync(path.join(rootDir, 'package.json'), 'utf8'));
  const baseVersion = pkg.version;
  const uiVersion = String(args['ui-version'] || `${baseVersion}-ui.${Math.floor(Date.now() / 1000)}`);
  const minAppVersion = String(args['min-app'] || baseVersion);
  const maxAppVersion = String(args['max-app'] || baseVersion);
  const downloadUrl = String(
    args.url ||
      `https://github.com/${DEFAULT_REPO_SLUG}/releases/download/${DEFAULT_RELEASE_TAG}/dist.zip`,
  );

  let privateKeyPem = normalizePemInput(process.env.UI_PATCH_SIGNING_PRIVATE_KEY);
  if (!privateKeyPem && args['key-file']) {
    privateKeyPem = fs.readFileSync(path.resolve(String(args['key-file'])), 'utf8');
  }
  if (!privateKeyPem) {
    throw new Error(
      'Missing UI_PATCH_SIGNING_PRIVATE_KEY env var (or --key-file path) to sign ui-manifest.json.',
    );
  }

  const entries = collectFiles(distDir);
  const zipBuf = createZipBuffer(entries);
  const sha256 = crypto.createHash('sha256').update(zipBuf).digest('hex');

  const manifest = signManifest(
    {
      ui_version: uiVersion,
      min_app_version: minAppVersion,
      max_app_version: maxAppVersion,
      sha256,
      url: downloadUrl,
      pub_date: new Date().toISOString(),
    },
    privateKeyPem,
  );

  fs.mkdirSync(outDir, { recursive: true });
  const zipOutPath = path.join(outDir, 'dist.zip');
  const manifestOutPath = path.join(outDir, 'ui-manifest.json');
  fs.writeFileSync(zipOutPath, zipBuf);
  fs.writeFileSync(manifestOutPath, JSON.stringify(manifest, null, 2));

  console.log(`Packaged ${entries.length} files (${zipBuf.length} bytes) -> ${zipOutPath}`);
  console.log(`Signed ui-manifest.json (ui_version=${uiVersion}) -> ${manifestOutPath}`);
}

const isMain = process.argv[1] && path.resolve(process.argv[1]) === path.resolve(new URL(import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1'));

if (isMain) {
  const args = parseArgs(process.argv.slice(2));
  if (args['verify-self-test']) {
    runSelfTest();
  } else if (args.keygen) {
    runKeygen(typeof args.out === 'string' ? args.out : null);
  } else {
    runPackage(args);
  }
}
