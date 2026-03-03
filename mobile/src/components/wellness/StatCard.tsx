import { View, Text } from "react-native";
import type { WellnessStat } from "@/types/wellness";

interface Props {
  stat: WellnessStat;
}

export default function StatCard({ stat }: Props) {
  const isPositive = stat.label === "Fokusaika"
    ? stat.after > stat.before
    : stat.after < stat.before;

  return (
    <View
      className="rounded-2xl p-4 mb-3"
      style={{ backgroundColor: "#1a1a24" }}
    >
      <Text className="text-sm text-gray-400 mb-3">{stat.label}</Text>

      <View className="flex-row items-center justify-between">
        {/* Ennen */}
        <View className="items-center flex-1">
          <Text className="text-xs text-gray-500 mb-1">Ennen</Text>
          <Text className="text-xl font-bold text-gray-400">
            {stat.before}
          </Text>
          <Text className="text-xs text-gray-500">{stat.unit}</Text>
        </View>

        {/* Nuoli */}
        <View className="items-center px-4">
          <Text style={{ fontSize: 20, color: isPositive ? "#4ecdc4" : "#ff6b6b" }}>
            →
          </Text>
        </View>

        {/* Jälkeen */}
        <View className="items-center flex-1">
          <Text className="text-xs text-gray-500 mb-1">Jälkeen</Text>
          <Text
            className="text-xl font-bold"
            style={{ color: isPositive ? "#4ecdc4" : "#ff6b6b" }}
          >
            {stat.after}
          </Text>
          <Text className="text-xs text-gray-500">{stat.unit}</Text>
        </View>
      </View>

      {/* Parannus */}
      <View
        className="mt-3 py-2 rounded-lg items-center"
        style={{ backgroundColor: isPositive ? "#4ecdc410" : "#ff6b6b10" }}
      >
        <Text
          className="text-sm font-semibold"
          style={{ color: isPositive ? "#4ecdc4" : "#ff6b6b" }}
        >
          {isPositive ? "↓" : "↑"} {stat.improvement}% {isPositive ? "parannus" : "muutos"}
        </Text>
      </View>
    </View>
  );
}
