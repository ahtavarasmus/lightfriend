import React, { useState, useRef, useCallback } from 'react';
import {
  View,
  FlatList,
  StyleSheet,
  KeyboardAvoidingView,
  Platform,
  Text,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import ChatMessage from '../components/ChatMessage';
import ChatInput from '../components/ChatInput';
import { sendMessage, sendMessageWithImage } from '../api/chat';
import { colors } from '../theme/colors';
import { typography } from '../theme/typography';

interface Message {
  id: string;
  content: string;
  role: 'user' | 'assistant';
}

export default function ChatScreen() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [sending, setSending] = useState(false);
  const flatListRef = useRef<FlatList>(null);
  const insets = useSafeAreaInsets();

  const addMessage = useCallback((content: string, role: 'user' | 'assistant') => {
    setMessages(prev => [...prev, { id: Date.now().toString() + role, content, role }]);
  }, []);

  const handleSend = useCallback(async (text: string) => {
    addMessage(text, 'user');
    setSending(true);
    try {
      const response = await sendMessage(text);
      addMessage(response.message, 'assistant');
    } catch (e: any) {
      addMessage(`Error: ${e.message}`, 'assistant');
    } finally {
      setSending(false);
    }
  }, [addMessage]);

  const handleSendWithImage = useCallback(async (text: string, imageUri: string) => {
    addMessage(`[Image] ${text}`, 'user');
    setSending(true);
    try {
      const response = await sendMessageWithImage(text, imageUri);
      addMessage(response.message, 'assistant');
    } catch (e: any) {
      addMessage(`Error: ${e.message}`, 'assistant');
    } finally {
      setSending(false);
    }
  }, [addMessage]);

  const renderItem = useCallback(({ item }: { item: Message }) => (
    <ChatMessage content={item.content} role={item.role} />
  ), []);

  return (
    <KeyboardAvoidingView
      style={styles.container}
      behavior={Platform.OS === 'ios' ? 'padding' : undefined}
      keyboardVerticalOffset={Platform.OS === 'ios' ? 90 : 0}
    >
      {messages.length === 0 ? (
        <View style={styles.emptyState}>
          <Text style={styles.emptyTitle}>Lightfriend</Text>
          <Text style={styles.emptySubtitle}>Your AI assistant. Ask me anything.</Text>
        </View>
      ) : (
        <FlatList
          ref={flatListRef}
          data={messages}
          renderItem={renderItem}
          keyExtractor={item => item.id}
          contentContainerStyle={[styles.list, { paddingBottom: 8 }]}
          onContentSizeChange={() => flatListRef.current?.scrollToEnd({ animated: true })}
          onLayout={() => flatListRef.current?.scrollToEnd({ animated: false })}
        />
      )}
      <ChatInput onSend={handleSend} onSendWithImage={handleSendWithImage} disabled={sending} />
      <View style={{ height: insets.bottom }} />
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: colors.background,
  },
  list: {
    paddingTop: 16,
  },
  emptyState: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    paddingHorizontal: 24,
  },
  emptyTitle: {
    ...typography.h1,
    color: colors.text,
    marginBottom: 8,
  },
  emptySubtitle: {
    ...typography.body,
    color: colors.textSecondary,
    textAlign: 'center',
  },
});
