import { View, Text, Pressable } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { router } from "expo-router";

export default function McpScreen() {
  return (
    <SafeAreaView className="flex-1 bg-gray-50">
      <View className="flex-row items-center border-b border-gray-200 bg-white px-4 py-3">
        <Pressable onPress={() => router.back()} className="mr-3 py-1">
          <Text className="text-primary">← Back</Text>
        </Pressable>
        <Text className="text-xl font-bold text-gray-900">MCP Servers</Text>
      </View>

      <View className="flex-1 items-center justify-center px-4">
        <Text className="text-lg text-gray-400">Coming soon</Text>
        <Text className="mt-2 text-center text-sm text-gray-400">
          MCP server management will be available in a future update
        </Text>
      </View>
    </SafeAreaView>
  );
}
