import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export const DEFAULT_GEMINI_MODEL = 'gemini-3.6-flash';
export const DEFAULT_GROQ_MODEL = 'openai/gpt-oss-120b';

export const DEFAULT_INTERVIEW_SYSTEM_PROMPT =
  'You are the interviewee in a live job interview. Respond directly in the first person ("I") using your resume context and the conversation accurately. Keep every answer short, natural, confident, and straight to the point—ready to be spoken aloud without filler or unnecessary preamble. When a question, coding problem, or screen capture is shown, give the direct answer and concise explanation as the candidate.';

export const PROVIDER_ITEMS = [
  { value: 'groq', name: 'Groq Cloud', tag: 'Recommended' },
  { value: 'gemini', name: 'Google Gemini', tag: 'Not Recommended' },
];

// Active Google AI Studio Free Tier chat/multimodal models (verified against aistudio.google.com/docs/models & /docs/pricing)
export const GEMINI_MODEL_ITEMS = [
  { value: 'gemini-3.6-flash', name: 'gemini-3.6-flash', tag: 'Recommended • Free' },
  { value: 'gemini-3.8-flash', name: 'gemini-3.8-flash', tag: 'Latest • Free' },
  { value: 'gemini-3.5-flash-lite', name: 'gemini-3.5-flash-lite', tag: 'Ultra-Fast • Free' },
  { value: 'gemini-3.1-flash-lite', name: 'gemini-3.1-flash-lite', tag: 'Fast • Free' },
  { value: 'gemini-2.5-flash-lite', name: 'gemini-2.5-flash-lite', tag: 'Legacy • Free' },
];

const OBSOLETE_GEMINI_MODELS = new Map([
  ['gemini-1.5-flash', 'gemini-3.6-flash'],
  ['gemini-1.5-pro', 'gemini-3.6-flash'],
  ['gemini-2.0-flash', 'gemini-3.6-flash'],
  ['gemini-2.0-flash-lite', 'gemini-3.5-flash-lite'],
  ['gemini-2.5-flash', 'gemini-3.6-flash'],
  ['gemini-2.5-pro', 'gemini-3.6-flash'],
  ['gemini-3-pro-preview', 'gemini-3.6-flash'],
  ['gemini-3.1-flash-lite-preview', 'gemini-3.5-flash-lite'],
]);

export function normalizeGeminiModel(modelId) {
  const clean = String(modelId || '').trim();
  if (!clean) return DEFAULT_GEMINI_MODEL;
  return OBSOLETE_GEMINI_MODELS.get(clean) || clean;
}

export function isGroqVisionModel(modelId) {
  const lower = String(modelId || '').toLowerCase();
  return (
    lower.includes('vision') ||
    lower.includes('vl') ||
    lower.includes('llama-4') ||
    lower.includes('qwen3.8') ||
    lower.includes('muse-spark') ||
    lower.includes('llava') ||
    lower.includes('pixtral')
  );
}

export function formatGroqModelItems(models) {
  return (models || []).map((m) => ({
    value: m,
    name: m,
    tag: isGroqVisionModel(m) ? 'Vision' : 'Text + OCR',
  }));
}

export function isLlmConfigured(config) {
  if (!config) return false;
  return config.active_provider === 'groq'
    ? Boolean(config.has_groq_key)
    : Boolean(config.has_gemini_key);
}

/**
 * ChatService
 * Encapsulates backend communication, whether operating within Tauri desktop
 * or over HTTP on the remote mobile viewer.
 */
export class ChatService {
  constructor({ authPin = '' } = {}) {
    this.authPin = authPin;
    this.isTauri = typeof window !== 'undefined' && Boolean(window.__TAURI_INTERNALS__);
  }

  setAuthPin(pin) {
    this.authPin = pin || '';
  }

  getAuthQuery() {
    return this.authPin ? '?pass=' + encodeURIComponent(this.authPin) : '';
  }

  async getConfig() {
    if (this.isTauri) {
      try {
        return await invoke('get_llm_config');
      } catch (e) {
        console.warn('Tauri invoke get_llm_config failed, falling back to HTTP:', e);
      }
    }

    const res = await fetch('/api/llm/config' + this.getAuthQuery());
    if (!res.ok) {
      throw new Error(`Failed to load LLM config (HTTP ${res.status})`);
    }
    return await res.json();
  }

  async saveConfig(config) {
    if (this.isTauri) {
      try {
        return await invoke('set_llm_config', { config });
      } catch (e) {
        console.warn('Tauri invoke set_llm_config failed, falling back to HTTP:', e);
      }
    }

    const res = await fetch('/api/llm/config' + this.getAuthQuery(), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(config),
    });
    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      throw new Error(err.error || `Failed to update LLM config (HTTP ${res.status})`);
    }
    const data = await res.json();
    return data.config;
  }

  async clearApiKey(provider) {
    if (!this.isTauri) {
      throw new Error('Clearing API keys is only available in the desktop app.');
    }
    return await invoke('clear_llm_api_key', { provider });
  }

  async uploadResume(file) {
    if (!this.isTauri) {
      throw new Error('Resume upload is only available in the desktop app.');
    }
    if (!file) {
      throw new Error('No file selected.');
    }
    if (file.size > 10 * 1024 * 1024) {
      throw new Error('Resume file is too large (10 MB maximum).');
    }
    const buf = await file.arrayBuffer();
    const bytes = Array.from(new Uint8Array(buf));
    return await invoke('upload_resume', { filename: file.name, bytes });
  }

  async clearResume() {
    if (!this.isTauri) {
      throw new Error('Clearing resume is only available in the desktop app.');
    }
    return await invoke('clear_resume');
  }

  async saveJobDescription(jobDescription) {
    if (!this.isTauri) {
      throw new Error('Saving job description is only available in the desktop app.');
    }
    return await invoke('set_job_description', {
      jobDescription: String(jobDescription || ''),
    });
  }

  async getGroqModels(apiKey = null) {
    if (this.isTauri) {
      try {
        const res = await invoke('get_groq_models', { apiKey: apiKey || null });
        if (Array.isArray(res) && res.length > 0) {
          return res;
        }
      } catch (e) {
        console.warn('Tauri invoke get_groq_models failed, trying HTTP:', e);
      }
    }

    try {
      const res = await fetch('/api/llm/groq-models' + this.getAuthQuery());
      if (res.ok) {
        const data = await res.json();
        if (data.success && Array.isArray(data.models) && data.models.length > 0) {
          return data.models;
        }
      }
    } catch (e) {
      console.warn('Fetch /api/llm/groq-models failed:', e);
    }

    // Direct fetch fallback if apiKey is provided
    if (apiKey) {
      try {
        const res = await fetch('https://api.groq.com/openai/v1/models', {
          headers: {
            Authorization: `Bearer ${apiKey.trim()}`,
            'Content-Type': 'application/json',
          },
        });
        if (res.ok) {
          const data = await res.json();
          if (Array.isArray(data.data)) {
            return data.data
              .filter((m) => {
                const lower = String(m.id || '').toLowerCase();
                return (
                  m.active !== false &&
                  !lower.startsWith('whisper') &&
                  !lower.startsWith('tts') &&
                  !lower.includes('orpheus') &&
                  !lower.includes('prompt-guard') &&
                  !lower.includes('safeguard')
                );
              })
              .map((m) => m.id)
              .sort((a, b) => Number(isGroqVisionModel(b)) - Number(isGroqVisionModel(a)) || a.localeCompare(b));
          }
        }
      } catch (e) {
        console.warn('Direct fetch from Groq models API failed:', e);
      }
    }

    // Fallback list of models
    return [
      'qwen/qwen3.8-27b',
      'openai/gpt-oss-120b',
      'openai/gpt-oss-20b',
      'allam-2-7b',
    ];
  }

  async runOcr(screenshotId) {
    if (this.isTauri) {
      try {
        const res = await invoke('run_ocr', { screenshotId: screenshotId || null });
        if (!res.success) {
          throw new Error(res.error || 'Failed to extract OCR text');
        }
        return res.text;
      } catch (e) {
        console.warn('Tauri invoke run_ocr failed, falling back to HTTP:', e);
      }
    }

    const res = await fetch('/api/llm/ocr' + this.getAuthQuery(), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ screenshot_id: screenshotId || null }),
    });
    const data = await res.json();
    if (!res.ok || !data.success) {
      throw new Error(data.error || `OCR error (HTTP ${res.status})`);
    }
    return data.text;
  }

  _buildChatPayload({ messages, screenshotId, useOcr, provider, model }) {
    return {
      messages: messages.map((m) => (m.toApiPayload ? m.toApiPayload() : m)),
      screenshot_id: screenshotId || null,
      use_ocr: Boolean(useOcr),
      provider: provider || null,
      model: model || null,
    };
  }

  async sendChat({ messages, screenshotId, useOcr, provider, model }) {
    const payload = this._buildChatPayload({ messages, screenshotId, useOcr, provider, model });

    if (this.isTauri) {
      try {
        const res = await invoke('send_llm_chat', { request: payload });
        return res;
      } catch (e) {
        console.warn('Tauri invoke send_llm_chat failed, falling back to HTTP:', e);
      }
    }

    const res = await fetch('/api/llm/chat' + this.getAuthQuery(), {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(payload),
    });
    const data = await res.json();
    if (!res.ok || !data.success) {
      throw new Error(data.error || `LLM request failed (HTTP ${res.status})`);
    }
    return data;
  }

  async sendChatStream({ messages, screenshotId, useOcr, provider, model, onChunk }) {
    const payload = this._buildChatPayload({ messages, screenshotId, useOcr, provider, model });

    if (!this.isTauri) {
      return this.sendChat({ messages, screenshotId, useOcr, provider, model });
    }

    const streamId = `stream_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
    let unlisten = null;
    try {
      unlisten = await listen('llm://stream', ({ payload: chunkEvent }) => {
        if (chunkEvent && chunkEvent.stream_id === streamId && chunkEvent.delta) {
          onChunk?.(chunkEvent.delta);
        }
      });

      return await invoke('send_llm_chat_stream', {
        request: payload,
        streamId,
      });
    } finally {
      if (typeof unlisten === 'function') {
        unlisten();
      }
    }
  }
}
