import { WS_URL } from "@/constants/config";
import { useAuthStore } from "@/stores/authStore";
import type { ChatMessage } from "@/types/api";

type MessageHandler = (msg: ChatMessage) => void;

const PING_INTERVAL = 30_000;
const RECONNECT_BASE_DELAY = 1_000;
const RECONNECT_MAX_DELAY = 30_000;

class WebSocketManager {
  private ws: WebSocket | null = null;
  private listeners = new Set<MessageHandler>();
  private pingTimer: ReturnType<typeof setInterval> | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectAttempts = 0;
  private intentionalClose = false;

  connect() {
    if (this.ws?.readyState === WebSocket.OPEN) return;

    const token = useAuthStore.getState().accessToken;
    if (!token) return;

    this.intentionalClose = false;
    const url = `${WS_URL}/api/ws?token=${encodeURIComponent(token)}`;

    this.ws = new WebSocket(url);

    this.ws.onopen = () => {
      this.reconnectAttempts = 0;
      this.startPing();
    };

    this.ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data) as ChatMessage;
        this.listeners.forEach((fn) => fn(msg));
      } catch {
        // ignore malformed messages
      }
    };

    this.ws.onclose = () => {
      this.stopPing();
      if (!this.intentionalClose) {
        this.scheduleReconnect();
      }
    };

    this.ws.onerror = () => {
      // onclose will fire after onerror
    };
  }

  disconnect() {
    this.intentionalClose = true;
    this.stopPing();
    this.clearReconnect();
    this.ws?.close();
    this.ws = null;
  }

  send(message: string) {
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify({ type: "chat", message }));
    }
  }

  subscribe(handler: MessageHandler): () => void {
    this.listeners.add(handler);
    return () => {
      this.listeners.delete(handler);
    };
  }

  private startPing() {
    this.stopPing();
    this.pingTimer = setInterval(() => {
      if (this.ws?.readyState === WebSocket.OPEN) {
        this.ws.send(JSON.stringify({ type: "ping" }));
      }
    }, PING_INTERVAL);
  }

  private stopPing() {
    if (this.pingTimer) {
      clearInterval(this.pingTimer);
      this.pingTimer = null;
    }
  }

  private scheduleReconnect() {
    this.clearReconnect();
    const delay = Math.min(
      RECONNECT_BASE_DELAY * Math.pow(2, this.reconnectAttempts),
      RECONNECT_MAX_DELAY,
    );
    this.reconnectAttempts++;
    this.reconnectTimer = setTimeout(() => this.connect(), delay);
  }

  private clearReconnect() {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
  }
}

export const wsManager = new WebSocketManager();
