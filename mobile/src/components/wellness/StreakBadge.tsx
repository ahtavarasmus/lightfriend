import { View, Text } from "react-native";

interface Props {
  streak: number;
  label?: string;
}

export default function StreakBadge({ streak, label = "Streak" }: Props) {
  return (
    <View
      className="flex-row items-center px-4 py-3 rounded-2xl"
      style={{ backgroundColor: "#1a1a24" }}
    >
      <Text style={{ fontSize: 24 }}>🔥</Text>
      <View className="ml-3">
        <Text className="text-2xl font-bold text-white">
          {streak} pv
        </Text>
        <Text className="text-xs text-gray-400">{label}</Text>
      </View>
    </View>
  );
}
