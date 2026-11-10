/**
 * Safe Markdown-to-HTML formatter for chat replies.
 * Shields fenced code blocks and inline code before inline formatting so code
 * content and attributes (like Copy button payloads) are never corrupted.
 */

const HEADING_CLASSES = {
  1: 'text-[15px] font-bold text-white mt-3 first:mt-0 mb-1.5 tracking-tight',
  2: 'text-sm font-bold text-white mt-3 first:mt-0 mb-1.5',
  3: 'text-[13.5px] font-semibold text-white mt-2.5 first:mt-0 mb-1',
  4: 'text-[13px] font-semibold text-zinc-100 mt-2 first:mt-0 mb-1',
  5: 'text-xs font-semibold text-zinc-200 mt-2 first:mt-0 mb-1',
  6: 'text-xs font-medium text-zinc-300 mt-2 first:mt-0 mb-1',
};

const TABLE_SEPARATOR_RE = /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/;
const CODEBLOCK_TOKEN_RE = /^\u0000CODEBLOCK_(\d+)\u0000$/;
const ORDERED_ITEM_RE = /^\s*(\d+)\.\s+(.*)$/;
const UNORDERED_ITEM_RE = /^\s*[-*+]\s+(.*)$/;
const HEADING_RE = /^\s*(#{1,6})\s+(.*)$/;
const HR_RE = /^\s*(?:-{3,}|\*{3,}|_{3,})\s*$/;
const BLOCKQUOTE_RE = /^\s*>\s?(.*)$/;

function escapeHtml(str) {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function renderCodeBlock(lang, rawCode) {
  const cleanCode = rawCode.replace(/\n$/, '');
  const languageLabel = lang ? lang.trim().toUpperCase() : 'CODE';
  return `<div class="code-block my-2.5 w-full rounded-lg overflow-hidden border border-zinc-800 bg-zinc-950 font-mono text-xs shadow-inner"><div class="flex items-center justify-between px-3 py-1.5 bg-zinc-900/90 border-b border-zinc-800 text-[10px] text-zinc-400 font-semibold tracking-wider leading-none"><span>${escapeHtml(languageLabel)}</span><button type="button" class="copy-btn text-zinc-300 hover:text-white transition-colors cursor-pointer px-2 py-0.5 rounded bg-zinc-800 hover:bg-zinc-700 leading-none" data-code="${encodeURIComponent(cleanCode)}">Copy</button></div><pre class="p-3 m-0 overflow-x-auto text-zinc-100 font-mono text-[12px] leading-relaxed"><code>${escapeHtml(cleanCode)}</code></pre></div>`;
}

function formatInline(rawText) {
  if (!rawText) return '';

  const inlineCodes = [];
  let text = rawText.replace(/`([^`\n]+)`/g, (_, inner) => {
    const idx = inlineCodes.length;
    inlineCodes.push(
      `<code class="px-1.5 py-0.5 rounded bg-zinc-800/90 border border-zinc-700/60 text-zinc-100 font-mono text-[11.5px] break-words">${escapeHtml(inner)}</code>`
    );
    return `\u0000INLINECODE_${idx}\u0000`;
  });

  text = escapeHtml(text);

  // Links [label](https://...)
  text = text.replace(
    /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
    '<a href="$2" target="_blank" rel="noopener noreferrer" class="text-white underline decoration-zinc-500 hover:decoration-white underline-offset-2 break-all">$1</a>'
  );

  // Strikethrough, Bold+Italic, Bold, and Word-Safe Italic (*...* only so snake_case stays intact)
  text = text.replace(/~~([^~\n]+)~~/g, '<del class="line-through text-zinc-500">$1</del>');
  text = text.replace(/\*\*\*([^*\n]+)\*\*\*/g, '<strong class="font-semibold italic text-white">$1</strong>');
  text = text.replace(/\*\*([^*\n]+)\*\*/g, '<strong class="font-semibold text-white">$1</strong>');
  text = text.replace(
    /(^|[^\w*])\*([^\s*](?:[^*\n]*?[^\s*])?)\*(?=[^\w*]|$)/g,
    '$1<em class="italic text-zinc-300">$2</em>'
  );

  return text.replace(/\u0000INLINECODE_(\d+)\u0000/g, (_, idx) => inlineCodes[Number(idx)] || '');
}

function splitTableRow(line) {
  let trimmed = line.trim();
  if (trimmed.startsWith('|')) trimmed = trimmed.slice(1);
  if (trimmed.endsWith('|') && !trimmed.endsWith('\\|')) trimmed = trimmed.slice(0, -1);

  const cells = [];
  let current = '';
  let inBackticks = false;

  for (let i = 0; i < trimmed.length; i++) {
    const ch = trimmed[i];
    if (ch === '`' && trimmed[i - 1] !== '\\') {
      inBackticks = !inBackticks;
      current += ch;
    } else if (ch === '|' && !inBackticks && trimmed[i - 1] !== '\\') {
      cells.push(current.trim());
      current = '';
    } else if (ch === '\\' && trimmed[i + 1] === '|') {
      current += '|';
      i++;
    } else {
      current += ch;
    }
  }
  cells.push(current.trim());
  return cells;
}

function isTableStart(lines, idx) {
  return (
    idx + 1 < lines.length &&
    lines[idx].includes('|') &&
    TABLE_SEPARATOR_RE.test(lines[idx + 1])
  );
}

function isBlockStarter(lines, idx) {
  const line = lines[idx];
  const trimmed = line.trim();
  if (!trimmed) return true;
  if (CODEBLOCK_TOKEN_RE.test(trimmed)) return true;
  if (HEADING_RE.test(line) || HR_RE.test(line) || BLOCKQUOTE_RE.test(line)) return true;
  if (ORDERED_ITEM_RE.test(line) || UNORDERED_ITEM_RE.test(line)) return true;
  if (isTableStart(lines, idx)) return true;
  return false;
}

export function formatMarkdown(text) {
  if (!text) return '';

  const normalized = String(text).replace(/\r\n?/g, '\n').replace(/\u0000/g, '');
  const codeBlocks = [];

  // Extract fenced code blocks before any Markdown/HTML transforms touch them
  let body = normalized.replace(/```([a-zA-Z0-9_+#.-]*)[ \t]*\n([\s\S]*?)```/g, (_, lang, code) => {
    const idx = codeBlocks.length;
    codeBlocks.push(renderCodeBlock(lang, code));
    return `\n\n\u0000CODEBLOCK_${idx}\u0000\n\n`;
  });

  // Handle unclosed trailing code fence gracefully (e.g. truncated reply)
  body = body.replace(/(?:^|\n)```([a-zA-Z0-9_+#.-]*)[ \t]*\n([\s\S]+)$/, (_, lang, code) => {
    const idx = codeBlocks.length;
    codeBlocks.push(renderCodeBlock(lang, code));
    return `\n\n\u0000CODEBLOCK_${idx}\u0000\n\n`;
  });

  const lines = body.split('\n');
  const out = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];
    const trimmed = line.trim();

    if (!trimmed) {
      i++;
      continue;
    }

    const tokenMatch = trimmed.match(CODEBLOCK_TOKEN_RE);
    if (tokenMatch) {
      out.push(codeBlocks[Number(tokenMatch[1])] || '');
      i++;
      continue;
    }

    if (HR_RE.test(line)) {
      out.push('<hr class="my-3 border-zinc-800" />');
      i++;
      continue;
    }

    const headingMatch = line.match(HEADING_RE);
    if (headingMatch) {
      const level = headingMatch[1].length;
      const cls = HEADING_CLASSES[level] || HEADING_CLASSES[4];
      out.push(`<h${level} class="${cls}">${formatInline(headingMatch[2].trim())}</h${level}>`);
      i++;
      continue;
    }

    if (isTableStart(lines, i)) {
      const headers = splitTableRow(lines[i]);
      const aligns = splitTableRow(lines[i + 1]).map((cell) => {
        if (cell.startsWith(':') && cell.endsWith(':')) return 'text-center';
        if (cell.endsWith(':')) return 'text-right';
        return 'text-left';
      });
      i += 2;

      const rows = [];
      while (
        i < lines.length &&
        lines[i].trim() &&
        lines[i].includes('|') &&
        !CODEBLOCK_TOKEN_RE.test(lines[i].trim())
      ) {
        rows.push(splitTableRow(lines[i]));
        i++;
      }

      const thead = `<thead class="bg-zinc-950/90 border-b border-zinc-800 text-zinc-100 font-semibold"><tr>${headers
        .map((h, col) => `<th class="px-3 py-2 ${aligns[col] || 'text-left'} align-top">${formatInline(h)}</th>`)
        .join('')}</tr></thead>`;

      const tbody = rows.length
        ? `<tbody class="divide-y divide-zinc-800/80 text-zinc-300">${rows
            .map(
              (row) =>
                `<tr class="hover:bg-white/[0.02]">${headers
                  .map(
                    (_, col) =>
                      `<td class="px-3 py-2 ${aligns[col] || 'text-left'} align-top leading-relaxed">${formatInline(
                        row[col] ?? ''
                      )}</td>`
                  )
                  .join('')}</tr>`
            )
            .join('')}</tbody>`
        : '';

      out.push(
        `<div class="my-2.5 w-full overflow-x-auto rounded-lg border border-zinc-800 bg-zinc-950/60"><table class="w-full border-collapse text-xs">${thead}${tbody}</table></div>`
      );
      continue;
    }

    if (BLOCKQUOTE_RE.test(line)) {
      const quoteLines = [];
      while (i < lines.length && BLOCKQUOTE_RE.test(lines[i])) {
        const m = lines[i].match(BLOCKQUOTE_RE);
        if (m[1].trim()) quoteLines.push(formatInline(m[1]));
        i++;
      }
      out.push(
        `<blockquote class="my-2 border-l-2 border-zinc-500 pl-3 py-1.5 text-zinc-300 italic bg-white/[0.03] rounded-r space-y-1">${quoteLines.join('<br />')}</blockquote>`
      );
      continue;
    }

    const orderedFirst = line.match(ORDERED_ITEM_RE);
    if (orderedFirst) {
      const startNum = Number(orderedFirst[1]);
      const items = [];
      while (i < lines.length) {
        const m = lines[i].match(ORDERED_ITEM_RE);
        if (m) {
          let itemText = m[2].trim();
          i++;
          while (i < lines.length && lines[i].trim() && !isBlockStarter(lines, i)) {
            itemText += ` ${lines[i].trim()}`;
            i++;
          }
          items.push(`<li class="leading-relaxed">${formatInline(itemText)}</li>`);
        } else {
          break;
        }
      }
      const startAttr = startNum !== 1 ? ` start="${startNum}"` : '';
      out.push(
        `<ol${startAttr} class="my-2 pl-5 list-decimal space-y-1.5 text-zinc-200 marker:text-zinc-400 marker:font-mono marker:text-[11px]">${items.join('')}</ol>`
      );
      continue;
    }

    if (UNORDERED_ITEM_RE.test(line)) {
      const items = [];
      while (i < lines.length) {
        const m = lines[i].match(UNORDERED_ITEM_RE);
        if (m) {
          let itemText = m[1].trim();
          i++;
          while (i < lines.length && lines[i].trim() && !isBlockStarter(lines, i)) {
            itemText += ` ${lines[i].trim()}`;
            i++;
          }
          items.push(`<li class="leading-relaxed">${formatInline(itemText)}</li>`);
        } else {
          break;
        }
      }
      out.push(
        `<ul class="my-2 pl-5 list-disc space-y-1 text-zinc-200 marker:text-zinc-500">${items.join('')}</ul>`
      );
      continue;
    }

    const paraLines = [];
    while (i < lines.length && lines[i].trim() && !isBlockStarter(lines, i)) {
      paraLines.push(formatInline(lines[i]));
      i++;
    }
    if (paraLines.length > 0) {
      out.push(
        `<p class="my-1.5 leading-relaxed text-zinc-200 first:mt-0 last:mb-0">${paraLines.join('<br />')}</p>`
      );
    } else {
      i++;
    }
  }

  return out.join('');
}
