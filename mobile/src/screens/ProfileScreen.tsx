import React, { useState, useCallback } from 'react';
import {
  View,
  Text,
  StyleSheet,
  ScrollView,
  TouchableOpacity,
  ActivityIndicator,
  RefreshControl,
  Alert,
} from 'react-native';
import { useFocusEffect } from '@react-navigation/native';
import { Ionicons } from '@expo/vector-icons';
import { colors } from '../theme/colors';
import { typography } from '../theme/typography';
import { getProfile } from '../api/profile';
import { useAuth } from '../auth/AuthContext';
import type { UserProfile } from '../api/types';

export default function ProfileScreen() {
  const { logout } = useAuth();
  const [profile, setProfile] = useState<UserProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);

  const fetchProfile = useCallback(async () => {
    try {
      const data = await getProfile();
      setProfile(data);
    } catch (e) {
      console.error('Profile fetch error:', e);
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  }, []);

  useFocusEffect(
    useCallback(() => {
      setLoading(true);
      fetchProfile();
    }, [fetchProfile])
  );

  const handleLogout = () => {
    Alert.alert('Sign Out', 'Are you sure you want to sign out?', [
      { text: 'Cancel', style: 'cancel' },
      { text: 'Sign Out', style: 'destructive', onPress: logout },
    ]);
  };

  if (loading && !profile) {
    return (
      <View style={styles.centered}>
        <ActivityIndicator size="large" color={colors.primary} />
      </View>
    );
  }

  const infoRows = [
    { icon: 'mail-outline' as const, label: 'Email', value: profile?.email },
    { icon: 'call-outline' as const, label: 'Phone', value: profile?.phone_number },
    { icon: 'globe-outline' as const, label: 'Timezone', value: profile?.timezone || 'Not set' },
    { icon: 'language-outline' as const, label: 'Language', value: profile?.language || 'English' },
    { icon: 'wallet-outline' as const, label: 'Credits', value: profile?.credits_left?.toFixed(2) },
    { icon: 'ribbon-outline' as const, label: 'Plan', value: profile?.plan_type || profile?.sub_tier || 'Free' },
  ];

  return (
    <ScrollView
      style={styles.container}
      contentContainerStyle={styles.content}
      refreshControl={<RefreshControl refreshing={refreshing} onRefresh={() => { setRefreshing(true); fetchProfile(); }} tintColor={colors.primary} />}
    >
      <View style={styles.card}>
        {infoRows.map((row, i) => (
          <View key={row.label} style={[styles.row, i < infoRows.length - 1 && styles.rowBorder]}>
            <Ionicons name={row.icon} size={20} color={colors.textSecondary} />
            <Text style={styles.label}>{row.label}</Text>
            <Text style={styles.value}>{row.value}</Text>
          </View>
        ))}
      </View>

      <TouchableOpacity style={styles.logoutButton} onPress={handleLogout}>
        <Ionicons name="log-out-outline" size={20} color={colors.error} />
        <Text style={styles.logoutText}>Sign Out</Text>
      </TouchableOpacity>
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
  card: {
    backgroundColor: colors.surface,
    borderRadius: 16,
    overflow: 'hidden',
    marginBottom: 24,
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    padding: 16,
    gap: 12,
  },
  rowBorder: {
    borderBottomWidth: 1,
    borderBottomColor: colors.border,
  },
  label: {
    ...typography.bodySmall,
    color: colors.textSecondary,
    width: 80,
  },
  value: {
    ...typography.body,
    color: colors.text,
    flex: 1,
    textAlign: 'right',
  },
  logoutButton: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 8,
    backgroundColor: colors.surface,
    borderRadius: 12,
    padding: 16,
  },
  logoutText: {
    ...typography.button,
    color: colors.error,
  },
});
