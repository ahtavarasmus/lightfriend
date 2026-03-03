import { View, Text } from "react-native";
import type { ChatEntry } from "@/hooks/useChat";

interface Props {
  entry: ChatEntry;
}

export function ChatMessageBubble({ entry }: Props) {
  const isUser = entry.role === "user";
  const isError = entry.role === "error";

  return (
    <View
      className={`mb-3 max-w-[85%] rounded-2xl px-4 py-3 ${
        isUser
          ? "self-end bg-primary"
          : isError
            ? "self-start bg-red-100"
            : "self-start bg-gray-100"
      }`}
    >
      <Text
        className={`text-base ${
          isUser ? "text-white" : isError ? "text-red-700" : "text-gray-900"
        }`}
      >
        {entry.text}
      </Text>
      {entry.creditsCharged != null && entry.creditsCharged > 0 && (
        <Text className="mt-1 text-xs text-gray-400">
          {entry.creditsCharged} credit{entry.creditsCharged !== 1 ? "s" : ""}
        </Text>
      )}
    </View>
  );
}
