import { useEffect, useRef, useCallback, useState } from "react";
import { wsManager } from "@/utils/websocket";
import { useAuthStore } from "@/stores/authStore";
import type { ChatMessage } from "@/types/api";

export interface ChatEntry {
  id: string;
  role: "user" | "assistant" | "error";
  text: string;
  creditsCharged?: number;
  media?: string | null;
  timestamp: Date;
}

export function useChat() {
  const [messages, setMessages] = useState<ChatEntry[]>([]);
  const [isConnected, setIsConnected] = useState(false);
  const isAuthenticated = useAuthStore((s) => s.isAuthenticated);
  const idCounter = useRef(0);

  useEffect(() => {
    if (!isAuthenticated) return;

    wsManager.connect();
    setIsConnected(true);

    const unsub = wsManager.subscribe((msg: ChatMessage) => {
      if (msg.type === "pong") return;

      if (msg.type === "chat_response") {
        setMessages((prev) => [
          ...prev,
          {
            id: String(++idCounter.current),
            role: "assistant",
            text: msg.message ?? "",
            creditsCharged: msg.credits_charged,
            media: msg.media,
            timestamp: new Date(),
          },
        ]);
      } else if (msg.type === "chat_error") {
        setMessages((prev) => [
          ...prev,
          {
            id: String(++idCounter.current),
            role: "error",
            text: msg.error ?? "Unknown error",
            timestamp: new Date(),
          },
        ]);
      }
    });

    return () => {
      unsub();
      wsManager.disconnect();
      setIsConnected(false);
    };
  }, [isAuthenticated]);

  const sendMessage = useCallback(
    (text: string) => {
      if (!text.trim()) return;

      setMessages((prev) => [
        ...prev,
        {
          id: String(++idCounter.current),
          role: "user",
          text,
          timestamp: new Date(),
        },
      ]);

      wsManager.send(text);
    },
    [],
  );

  const clearMessages = useCallback(() => {
    setMessages([]);
  }, []);

  return { messages, sendMessage, clearMessages, isConnected };
}
