import React, { useState, useCallback } from 'react';
import {
  View,
  Text,
  StyleSheet,
  ScrollView,
  RefreshControl,
  ActivityIndicator,
} from 'react-native';
import { useFocusEffect } from '@react-navigation/native';
import { Ionicons } from '@expo/vector-icons';
import { colors } from '../theme/colors';
import { typography } from '../theme/typography';
import { getDashboardSummary } from '../api/dashboard';
import TaskCard from '../components/TaskCard';
import type { DashboardSummary } from '../api/types';

export default function DashboardScreen() {
  const [data, setData] = useState<DashboardSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);

  const fetchData = useCallback(async () => {
    try {
      const summary = await getDashboardSummary();
      setData(summary);
    } catch (e) {
      console.error('Dashboard fetch error:', e);
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  useFocusEffect(
    useCallback(() => {
      setLoading(true);
      fetchData();
    }, [fetchData])
  );

  const onRefresh = () => {
    setRefreshing(true);
    fetchData();
  };

  if (loading && !data) {
    return (
      <View style={styles.centered}>
        <ActivityIndicator size="large" color={colors.primary} />
      </View>
    );
  }

  return (
    <ScrollView
      style={styles.container}
      contentContainerStyle={styles.content}
      refreshControl={<RefreshControl refreshing={refreshing} onRefresh={onRefresh} tintColor={colors.primary} />}
    >
      {/* Status Cards */}
      <View style={styles.statsRow}>
        <View style={styles.statCard}>
          <Ionicons name="notifications-outline" size={24} color={colors.warning} />
          <Text style={styles.statNumber}>{data?.attention_count ?? 0}</Text>
          <Text style={styles.statLabel}>Attention</Text>
        </View>
        <View style={styles.statCard}>
          <Ionicons
            name={data?.quiet_mode ? 'moon' : 'moon-outline'}
            size={24}
            color={data?.quiet_mode ? colors.primary : colors.textSecondary}
          />
          <Text style={styles.statNumber}>{data?.quiet_mode ? 'On' : 'Off'}</Text>
          <Text style={styles.statLabel}>Quiet Mode</Text>
        </View>
      </View>

      {/* Upcoming Tasks */}
      <Text style={styles.sectionTitle}>Upcoming Tasks</Text>
      {data?.upcoming_tasks && data.upcoming_tasks.length > 0 ? (
        data.upcoming_tasks.map(task => <TaskCard key={task.id} task={task} />)
      ) : (
        <View style={styles.emptyCard}>
          <Text style={styles.emptyText}>No upcoming tasks</Text>
        </View>
      )}

      {/* Upcoming Digests */}
      {data?.upcoming_digests && data.upcoming_digests.length > 0 && (
        <>
          <Text style={styles.sectionTitle}>Upcoming Digests</Text>
          {data.upcoming_digests.map((digest, i) => (
            <View key={i} style={styles.digestCard}>
              <Ionicons name="time-outline" size={18} color={colors.textSecondary} />
              <Text style={styles.digestText}>{digest}</Text>
            </View>
          ))}
        </>
      )}
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: colors.background,
  },
  content: {
    padding: 16,
  },
  centered: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    backgroundColor: colors.background,
  },
  statsRow: {
    flexDirection: 'row',
    gap: 12,
    marginBottom: 24,
  },
  statCard: {
    flex: 1,
    backgroundColor: colors.surface,
    borderRadius: 16,
    padding: 16,
    alignItems: 'center',
    gap: 4,
  },
  statNumber: {
    ...typography.h2,
    color: colors.text,
  },
  statLabel: {
    ...typography.caption,
    color: colors.textSecondary,
  },
  sectionTitle: {
    ...typography.h3,
    color: colors.text,
    marginBottom: 12,
  },
  emptyCard: {
    backgroundColor: colors.surface,
    borderRadius: 12,
    padding: 24,
    alignItems: 'center',
    marginBottom: 24,
  },
  emptyText: {
    ...typography.body,
    color: colors.textSecondary,
  },
  digestCard: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: colors.surface,
    borderRadius: 12,
    padding: 14,
    marginBottom: 8,
    gap: 10,
  },
  digestText: {
    ...typography.bodySmall,
    color: colors.text,
    flex: 1,
  },
});
