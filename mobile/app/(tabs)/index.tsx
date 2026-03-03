import { View, Text } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { ChatBox } from "@/components/ChatBox";
import { CreditsBadge } from "@/components/CreditsBadge";
import { useDashboardSummary } from "@/hooks/useDashboard";

export default function DashboardScreen() {
  const { data: summary } = useDashboardSummary();

  return (
    <SafeAreaView className="flex-1 bg-gray-50" edges={["top"]}>
      <View className="flex-row items-center justify-between border-b border-gray-200 bg-white px-4 py-3">
        <Text className="text-xl font-bold text-gray-900">Lightfriend</Text>
        {summary && <CreditsBadge credits={summary.credits_remaining} />}
      </View>
      <ChatBox />
    </SafeAreaView>
  );
}
