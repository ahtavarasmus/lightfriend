import React from 'react';
import { View, Text, TouchableOpacity, StyleSheet } from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import { colors } from '../theme/colors';
import { typography } from '../theme/typography';
import type { TriageItem } from '../api/types';

interface Props {
  item: TriageItem;
  onExecute: (id: number) => void;
  onSnooze: (id: number) => void;
  onDismiss: (id: number) => void;
}

export default function AttentionCard({ item, onExecute, onSnooze, onDismiss }: Props) {
  return (
    <View style={styles.card}>
      <View style={styles.header}>
        <View style={styles.sourceTag}>
          <Text style={styles.sourceText}>{item.source}</Text>
        </View>
        {item.sender && <Text style={styles.sender}>{item.sender}</Text>}
      </View>

      <Text style={styles.summary}>{item.summary}</Text>

      {item.suggested_response && (
        <View style={styles.suggestedContainer}>
          <Text style={styles.suggestedLabel}>Suggested response:</Text>
          <Text style={styles.suggestedText}>{item.suggested_response}</Text>
        </View>
      )}

      <View style={styles.actions}>
        <TouchableOpacity style={styles.actionButton} onPress={() => onExecute(item.id)}>
          <Ionicons name="send-outline" size={18} color={colors.primary} />
          <Text style={[styles.actionText, { color: colors.primary }]}>Reply</Text>
        </TouchableOpacity>

        <TouchableOpacity style={styles.actionButton} onPress={() => onSnooze(item.id)}>
          <Ionicons name="time-outline" size={18} color={colors.warning} />
          <Text style={[styles.actionText, { color: colors.warning }]}>Snooze</Text>
        </TouchableOpacity>

        <TouchableOpacity style={styles.actionButton} onPress={() => onDismiss(item.id)}>
          <Ionicons name="close-outline" size={18} color={colors.textSecondary} />
          <Text style={[styles.actionText, { color: colors.textSecondary }]}>Dismiss</Text>
        </TouchableOpacity>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    backgroundColor: colors.surface,
    borderRadius: 16,
    padding: 16,
    marginBottom: 12,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    marginBottom: 10,
  },
  sourceTag: {
    backgroundColor: colors.surfaceLight,
    paddingHorizontal: 10,
    paddingVertical: 4,
    borderRadius: 8,
  },
  sourceText: {
    ...typography.caption,
    color: colors.primary,
    textTransform: 'capitalize',
  },
  sender: {
    ...typography.caption,
    color: colors.textSecondary,
  },
  summary: {
    ...typography.body,
    color: colors.text,
    marginBottom: 12,
  },
  suggestedContainer: {
    backgroundColor: colors.surfaceLight,
    borderRadius: 10,
    padding: 12,
    marginBottom: 12,
  },
  suggestedLabel: {
    ...typography.caption,
    color: colors.textSecondary,
    marginBottom: 4,
  },
  suggestedText: {
    ...typography.bodySmall,
    color: colors.text,
    fontStyle: 'italic',
  },
  actions: {
    flexDirection: 'row',
    gap: 16,
  },
  actionButton: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 4,
  },
  actionText: {
    ...typography.bodySmall,
  },
});
