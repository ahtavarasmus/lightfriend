import React, { useState, useCallback } from 'react';
import {
  View,
  Text,
  StyleSheet,
  FlatList,
  RefreshControl,
  ActivityIndicator,
  Alert,
} from 'react-native';
import { useFocusEffect } from '@react-navigation/native';
import { colors } from '../theme/colors';
import { typography } from '../theme/typography';
import { getTriageItems, executeTriageItem, snoozeTriageItem, dismissTriageItem } from '../api/triage';
import AttentionCard from '../components/AttentionCard';
import type { TriageItem } from '../api/types';

export default function TriageScreen() {
  const [items, setItems] = useState<TriageItem[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);

  const fetchItems = useCallback(async () => {
    try {
      const data = await getTriageItems();
      setItems(data);
    } catch (e) {
      console.error('Triage fetch error:', e);
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  useFocusEffect(
    useCallback(() => {
      setLoading(true);
      fetchItems();
    }, [fetchItems])
  );

  const handleExecute = async (id: number) => {
    try {
      await executeTriageItem(id);
      setItems(prev => prev.filter(item => item.id !== id));
    } catch (e: any) {
      Alert.alert('Error', e.message);
    }
  };

  const handleSnooze = async (id: number) => {
    try {
      await snoozeTriageItem(id);
      setItems(prev => prev.filter(item => item.id !== id));
    } catch (e: any) {
      Alert.alert('Error', e.message);
    }
  };

  const handleDismiss = async (id: number) => {
    try {
      await dismissTriageItem(id);
      setItems(prev => prev.filter(item => item.id !== id));
    } catch (e: any) {
      Alert.alert('Error', e.message);
    }
  };

  if (loading && items.length === 0) {
    return (
      <View style={styles.centered}>
        <ActivityIndicator size="large" color={colors.primary} />
      </View>
    );
  }

  return (
    <FlatList
      style={styles.container}
      contentContainerStyle={items.length === 0 ? styles.emptyContainer : styles.list}
      data={items}
      keyExtractor={item => item.id.toString()}
      renderItem={({ item }) => (
        <AttentionCard
          item={item}
          onExecute={handleExecute}
          onSnooze={handleSnooze}
          onDismiss={handleDismiss}
        />
      )}
      refreshControl={<RefreshControl refreshing={refreshing} onRefresh={() => { setRefreshing(true); fetchItems(); }} tintColor={colors.primary} />}
      ListEmptyComponent={
        <View style={styles.emptyState}>
          <Text style={styles.emptyTitle}>All caught up!</Text>
          <Text style={styles.emptySubtitle}>No items need your attention</Text>
        </View>
      }
    />
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: colors.background,
  },
  list: {
    padding: 16,
  },
  centered: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    backgroundColor: colors.background,
  },
  emptyContainer: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
  },
  emptyState: {
    alignItems: 'center',
    paddingHorizontal: 24,
  },
  emptyTitle: {
    ...typography.h2,
    color: colors.text,
    marginBottom: 8,
  },
  emptySubtitle: {
    ...typography.body,
    color: colors.textSecondary,
  },
});
