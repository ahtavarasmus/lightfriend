import React from 'react';
import { View, Text, StyleSheet } from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import { colors } from '../theme/colors';
import { typography } from '../theme/typography';
import type { DashboardTask } from '../api/types';

interface Props {
  task: DashboardTask;
}

export default function TaskCard({ task }: Props) {
  return (
    <View style={styles.card}>
      <Ionicons
        name={task.completed ? 'checkmark-circle' : 'ellipse-outline'}
        size={20}
        color={task.completed ? colors.success : colors.textSecondary}
      />
      <View style={styles.content}>
        <Text style={[styles.title, task.completed && styles.titleCompleted]}>
          {task.title}
        </Text>
        {task.due_date && (
          <Text style={styles.dueDate}>{task.due_date}</Text>
        )}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: colors.surface,
    borderRadius: 12,
    padding: 14,
    marginBottom: 8,
    gap: 12,
  },
  content: {
    flex: 1,
  },
  title: {
    ...typography.body,
    color: colors.text,
  },
  titleCompleted: {
    textDecorationLine: 'line-through',
    color: colors.textSecondary,
  },
  dueDate: {
    ...typography.caption,
    color: colors.textMuted,
    marginTop: 2,
  },
});
