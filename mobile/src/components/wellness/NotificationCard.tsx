import { View, Text } from "react-native";
import type { WellnessNotification, CategoryConfig } from "@/types/wellness";

const CATEGORY_CONFIG: Record<string, CategoryConfig> = {
  bill: { key: "bill", label: "Lasku", emoji: "💰", color: "#ff6b6b" },
  calendar: { key: "calendar", label: "Aika", emoji: "📅", color: "#4ecdc4" },
  important: { key: "important", label: "Tärkeä", emoji: "⭐", color: "#ffd93d" },
  muted: { key: "muted", label: "Mykistetty", emoji: "🔇", color: "#666666" },
};

interface Props {
  notification: WellnessNotification;
  isProcessed?: boolean;
}

export default function NotificationCard({ notification, isProcessed }: Props) {
  const config = CATEGORY_CONFIG[notification.category];
  const opacity = isProcessed || notification.category === "muted" ? 0.4 : 1;

  return (
    <View
      className="rounded-xl p-4 mb-2"
      style={{
        backgroundColor: "#1a1a24",
        opacity,
        borderLeftWidth: 3,
        borderLeftColor: config.color,
      }}
    >
      <View className="flex-row items-center justify-between mb-1">
        <View className="flex-row items-center flex-1">
          <Text style={{ fontSize: 16 }}>{config.emoji}</Text>
          <Text
            className="text-base font-semibold ml-2 flex-1"
            style={{ color: config.color }}
            numberOfLines={1}
          >
            {notification.title}
          </Text>
        </View>
        {notification.amount != null && (
          <Text className="text-base font-bold" style={{ color: "#ff6b6b" }}>
            {notification.amount.toFixed(2)} €
          </Text>
        )}
      </View>
      <Text className="text-sm text-gray-400" numberOfLines={2}>
        {notification.body}
      </Text>
      <Text className="text-xs text-gray-500 mt-1">
        {notification.source}
      </Text>
    </View>
  );
}
