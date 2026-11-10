/**
 * Object-Oriented ChatMessage Entity
 * Encapsulates message attributes, formatting, serialization, and metadata.
 */
export class ChatMessage {
  constructor({
    id = null,
    role = 'user', // 'user' | 'assistant' | 'system'
    content = '',
    screenshotId = null,
    screenshotThumb = null,
    ocrText = null,
    provider = null,
    model = null,
    timestamp = null,
    error = null,
    isPending = false,
  } = {}) {
    this.id = id || 'msg_' + Date.now() + '_' + Math.random().toString(36).slice(2, 7);
    this.role = role;
    this.content = content;
    this.screenshotId = screenshotId;
    this.screenshotThumb = screenshotThumb;
    this.ocrText = ocrText;
    this.provider = provider;
    this.model = model;
    this.timestamp = timestamp || new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    this.error = error;
    this.isPending = isPending;
  }

  isUser() {
    return this.role === 'user';
  }

  isAssistant() {
    return this.role === 'assistant' || this.role === 'model';
  }

  hasScreenshot() {
    return Boolean(this.screenshotId);
  }

  hasOcr() {
    return Boolean(this.ocrText && this.ocrText.trim().length > 0);
  }

  toApiPayload() {
    return {
      role: this.role,
      content: this.content,
      ocr_text: this.ocrText || null,
    };
  }

  static fromJson(json) {
    return new ChatMessage(json);
  }
}
