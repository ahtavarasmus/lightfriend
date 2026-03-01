import { View, Text, ScrollView, TouchableOpacity, ActivityIndicator } from "react-native";
import { useRouter } from "expo-router";
import { LinearGradient } from "expo-linear-gradient";
import { SafeAreaView } from "react-native-safe-area-context";
import { useNotificationsByCategory, useBillsTotal } from "@/hooks/useNotifications";

export default function BillsScreen() {
  const router = useRouter();
  const { data: bills, isLoading } = useNotificationsByCategory("bill");
  const { data: total } = useBillsTotal();

  return (
    <LinearGradient colors={["#0a0a0f", "#111118", "#0a0a0f"]} style={{ flex: 1 }}>
      <SafeAreaView style={{ flex: 1 }}>
        <ScrollView className="flex-1 px-4 pt-4">
          <View className="flex-row items-center mb-2">
            <TouchableOpacity onPress={() => router.back()} className="mr-3">
              <Text className="text-2xl text-white">←</Text>
            </TouchableOpacity>
            <Text className="text-2xl font-bold" style={{ color: "#ff6b6b" }}>
              💰 Laskut
            </Text>
          </View>
          <Text className="text-sm text-gray-400 mb-6">
            Ilmoituksista poimitut laskut
          </Text>

          {/* Kokonaissumma */}
          {total != null && (
            <View
              className="rounded-2xl p-5 mb-6 items-center"
              style={{ backgroundColor: "#ff6b6b15" }}
            >
              <Text className="text-sm text-gray-400 mb-1">Yhteensä</Text>
              <Text className="text-3xl font-bold" style={{ color: "#ff6b6b" }}>
                {total.toFixed(2)} €
              </Text>
            </View>
          )}

          {isLoading ? (
            <ActivityIndicator color="#ff6b6b" className="mt-8" />
          ) : (
            bills?.map((bill) => (
              <View
                key={bill.id}
                className="rounded-xl p-4 mb-3 flex-row items-center justify-between"
                style={{ backgroundColor: "#1a1a24", borderLeftWidth: 3, borderLeftColor: "#ff6b6b" }}
              >
                <View className="flex-1">
                  <Text className="text-base font-semibold text-white">
                    {bill.title}
                  </Text>
                  <Text className="text-sm text-gray-400 mt-1">
                    {bill.body}
                  </Text>
                  <Text className="text-xs text-gray-500 mt-1">
                    {bill.source}
                  </Text>
                </View>
                <Text className="text-lg font-bold ml-3" style={{ color: "#ff6b6b" }}>
                  {bill.amount?.toFixed(2)} €
                </Text>
              </View>
            ))
          )}

          <View style={{ height: 32 }} />
        </ScrollView>
      </SafeAreaView>
    </LinearGradient>
  );
}
