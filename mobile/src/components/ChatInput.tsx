import React, { useState } from 'react';
import {
  View,
  TextInput,
  TouchableOpacity,
  StyleSheet,
  ActivityIndicator,
} from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import * as ImagePicker from 'expo-image-picker';
import { colors } from '../theme/colors';
import { typography } from '../theme/typography';

interface Props {
  onSend: (message: string) => void;
  onSendWithImage: (message: string, imageUri: string) => void;
  disabled?: boolean;
}

export default function ChatInput({ onSend, onSendWithImage, disabled }: Props) {
  const [text, setText] = useState('');

  const handleSend = () => {
    const trimmed = text.trim();
    if (!trimmed || disabled) return;
    onSend(trimmed);
    setText('');
  };

  const handlePickImage = async () => {
    const result = await ImagePicker.launchImageLibraryAsync({
      mediaTypes: ['images'],
      quality: 0.8,
    });

    if (!result.canceled && result.assets[0]) {
      const message = text.trim() || 'What is this?';
      onSendWithImage(message, result.assets[0].uri);
      setText('');
    }
  };

  return (
    <View style={styles.container}>
      <TouchableOpacity onPress={handlePickImage} style={styles.iconButton} disabled={disabled}>
        <Ionicons name="image-outline" size={24} color={colors.textSecondary} />
      </TouchableOpacity>

      <TextInput
        style={styles.input}
        placeholder="Message..."
        placeholderTextColor={colors.textMuted}
        value={text}
        onChangeText={setText}
        multiline
        maxLength={4000}
        editable={!disabled}
      />

      <TouchableOpacity
        onPress={handleSend}
        style={[styles.sendButton, (!text.trim() || disabled) && styles.sendButtonDisabled]}
        disabled={!text.trim() || disabled}
      >
        {disabled ? (
          <ActivityIndicator size="small" color={colors.text} />
        ) : (
          <Ionicons name="send" size={20} color={colors.text} />
        )}
      </TouchableOpacity>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    paddingHorizontal: 12,
    paddingVertical: 8,
    borderTopWidth: 1,
    borderTopColor: colors.border,
    backgroundColor: colors.background,
  },
  iconButton: {
    padding: 8,
    marginRight: 4,
  },
  input: {
    ...typography.body,
    flex: 1,
    color: colors.text,
    backgroundColor: colors.inputBackground,
    borderRadius: 20,
    paddingHorizontal: 16,
    paddingVertical: 10,
    maxHeight: 100,
  },
  sendButton: {
    backgroundColor: colors.primary,
    borderRadius: 20,
    width: 40,
    height: 40,
    alignItems: 'center',
    justifyContent: 'center',
    marginLeft: 8,
  },
  sendButtonDisabled: {
    opacity: 0.4,
  },
});
