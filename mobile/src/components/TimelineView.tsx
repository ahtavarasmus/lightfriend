import { View, Text, FlatList } from "react-native";
import type { DashboardItem } from "@/types/api";

interface Props {
  items: DashboardItem[];
}

export function TimelineView({ items }: Props) {
  if (items.length === 0) {
    return (
      <View className="items-center py-8">
        <Text className="text-gray-400">No items yet</Text>
      </View>
    );
  }

  return (
    <FlatList
      data={items}
      keyExtractor={(item) => String(item.id)}
      renderItem={({ item }) => (
        <View className="mb-3 rounded-xl bg-white p-4 shadow-sm">
          <View className="flex-row items-center justify-between">
            <Text className="text-xs font-medium uppercase text-gray-400">
              {item.source}
            </Text>
            <Text className="text-xs text-gray-400">
              {new Date(item.created_at).toLocaleTimeString()}
            </Text>
          </View>
          <Text className="mt-1 text-base font-semibold text-gray-900">
            {item.title}
          </Text>
          <Text className="mt-1 text-sm text-gray-600" numberOfLines={3}>
            {item.content}
          </Text>
        </View>
      )}
      showsVerticalScrollIndicator={false}
    />
  );
}
