import { useRef, useEffect, useState } from "react";
import {
  View,
  TextInput,
  FlatList,
  Pressable,
  Text,
  KeyboardAvoidingView,
  Platform,
} from "react-native";
import { ChatMessageBubble } from "./ChatMessage";
import { useChat } from "@/hooks/useChat";

export function ChatBox() {
  const { messages, sendMessage, isConnected } = useChat();
  const [input, setInput] = useState("");
  const flatListRef = useRef<FlatList>(null);

  useEffect(() => {
    if (messages.length > 0) {
      flatListRef.current?.scrollToEnd({ animated: true });
    }
  }, [messages.length]);

  const handleSend = () => {
    if (!input.trim()) return;
    sendMessage(input.trim());
    setInput("");
  };

  return (
    <KeyboardAvoidingView
      className="flex-1"
      behavior={Platform.OS === "ios" ? "padding" : "height"}
      keyboardVerticalOffset={100}
    >
      <FlatList
        ref={flatListRef}
        data={messages}
        keyExtractor={(item) => item.id}
        renderItem={({ item }) => <ChatMessageBubble entry={item} />}
        contentContainerStyle={{ padding: 16, flexGrow: 1, justifyContent: "flex-end" }}
        showsVerticalScrollIndicator={false}
        ListEmptyComponent={
          <View className="flex-1 items-center justify-center">
            <Text className="text-lg text-gray-400">
              Send a message to start chatting
            </Text>
          </View>
        }
      />

      <View className="flex-row items-center border-t border-gray-200 bg-white px-4 py-3">
        {!isConnected && (
          <View className="mr-2 h-2 w-2 rounded-full bg-red-500" />
        )}
        <TextInput
          className="mr-3 flex-1 rounded-full border border-gray-300 bg-gray-50 px-4 py-2 text-base"
          placeholder="Type a message..."
          value={input}
          onChangeText={setInput}
          onSubmitEditing={handleSend}
          returnKeyType="send"
          editable={isConnected}
        />
        <Pressable
          onPress={handleSend}
          disabled={!input.trim() || !isConnected}
          className={`rounded-full px-5 py-2 ${
            input.trim() && isConnected ? "bg-primary" : "bg-gray-300"
          }`}
        >
          <Text className="font-semibold text-white">Send</Text>
        </Pressable>
      </View>
    </KeyboardAvoidingView>
  );
}
