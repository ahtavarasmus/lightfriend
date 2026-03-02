import React, { useState, useCallback } from 'react';
import { createBottomTabNavigator } from '@react-navigation/bottom-tabs';
import { Ionicons } from '@expo/vector-icons';
import { useFocusEffect } from '@react-navigation/native';
import { colors } from '../theme/colors';
import ChatScreen from '../screens/ChatScreen';
import DashboardScreen from '../screens/DashboardScreen';
import TriageScreen from '../screens/TriageScreen';
import ProfileScreen from '../screens/ProfileScreen';
import { getDashboardSummary } from '../api/dashboard';

const Tab = createBottomTabNavigator();

export default function MainTabs() {
  const [attentionCount, setAttentionCount] = useState(0);

  // Fetch attention count for badge
  const fetchAttentionCount = useCallback(async () => {
    try {
      const summary = await getDashboardSummary();
      setAttentionCount(summary.attention_count);
    } catch {
      // Silently fail
    }
  }, []);

  useFocusEffect(
    useCallback(() => {
      fetchAttentionCount();
    }, [fetchAttentionCount])
  );

  return (
    <Tab.Navigator
      screenOptions={{
        tabBarStyle: {
          backgroundColor: colors.tabBar,
          borderTopColor: colors.tabBarBorder,
        },
        tabBarActiveTintColor: colors.primary,
        tabBarInactiveTintColor: colors.textMuted,
        headerStyle: {
          backgroundColor: colors.background,
        },
        headerTintColor: colors.text,
        headerShadowVisible: false,
      }}
    >
      <Tab.Screen
        name="Chat"
        component={ChatScreen}
        options={{
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="chatbubble-outline" size={size} color={color} />
          ),
          headerTitle: 'Lightfriend',
        }}
      />
      <Tab.Screen
        name="Dashboard"
        component={DashboardScreen}
        options={{
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="grid-outline" size={size} color={color} />
          ),
        }}
      />
      <Tab.Screen
        name="Triage"
        component={TriageScreen}
        options={{
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="alert-circle-outline" size={size} color={color} />
          ),
          tabBarBadge: attentionCount > 0 ? attentionCount : undefined,
          tabBarBadgeStyle: {
            backgroundColor: colors.badge,
            fontSize: 11,
          },
        }}
      />
      <Tab.Screen
        name="Profile"
        component={ProfileScreen}
        options={{
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="person-outline" size={size} color={color} />
          ),
        }}
      />
    </Tab.Navigator>
  );
}
