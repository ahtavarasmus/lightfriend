import { View, Text } from "react-native";
import type { WellnessNotification, CategoryConfig } from "@/types/wellness";
import NotificationCard from "./NotificationCard";

const CATEGORY_CONFIGS: CategoryConfig[] = [
  { key: "important", label: "Tärkeä", emoji: "⭐", color: "#ffd93d" },
  { key: "bill", label: "Lasku", emoji: "💰", color: "#ff6b6b" },
  { key: "calendar", label: "Aika", emoji: "📅", color: "#4ecdc4" },
  { key: "muted", label: "Mykistetty", emoji: "🔇", color: "#666666" },
];

interface Props {
  notifications: WellnessNotification[];
  processedIds: Set<string>;
}

export default function CategorySection({ notifications, processedIds }: Props) {
  return (
    <View>
      {CATEGORY_CONFIGS.map((config) => {
        const items = notifications.filter((n) => n.category === config.key);
        if (items.length === 0) return null;
        return (
          <View key={config.key} className="mb-6">
            <View className="flex-row items-center mb-3">
              <Text style={{ fontSize: 18 }}>{config.emoji}</Text>
              <Text
                className="text-lg font-bold ml-2"
                style={{ color: config.color }}
              >
                {config.label}
              </Text>
              <View
                className="ml-2 px-2 py-0.5 rounded-full"
                style={{ backgroundColor: config.color + "20" }}
              >
                <Text className="text-xs font-medium" style={{ color: config.color }}>
                  {items.length}
                </Text>
              </View>
            </View>
            {items.map((n) => (
              <NotificationCard
                key={n.id}
                notification={n}
                isProcessed={processedIds.has(n.id)}
              />
            ))}
          </View>
        );
      })}
    </View>
  );
}
