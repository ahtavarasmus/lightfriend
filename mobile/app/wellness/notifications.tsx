import { View, Text, ScrollView, TouchableOpacity, ActivityIndicator } from "react-native";
import { useRouter } from "expo-router";
import { LinearGradient } from "expo-linear-gradient";
import { SafeAreaView } from "react-native-safe-area-context";
import { useNotifications } from "@/hooks/useNotifications";
import { useWellnessStore } from "@/stores/wellnessStore";
import CategorySection from "@/components/wellness/CategorySection";

export default function NotificationsScreen() {
  const router = useRouter();
  const { data: notifications, isLoading } = useNotifications();
  const { processedNotificationIds, markAllProcessed } = useWellnessStore();

  function handleProcessAll() {
    if (!notifications) return;
    const ids = notifications.map((n) => n.id);
    markAllProcessed(ids);
  }

  const allProcessed =
    notifications?.every((n) => processedNotificationIds.has(n.id)) ?? false;

  return (
    <LinearGradient colors={["#0a0a0f", "#111118", "#0a0a0f"]} style={{ flex: 1 }}>
      <SafeAreaView style={{ flex: 1 }}>
        <ScrollView className="flex-1 px-4 pt-4">
          {/* Header */}
          <View className="flex-row items-center mb-2">
            <TouchableOpacity onPress={() => router.back()} className="mr-3">
              <Text className="text-2xl text-white">←</Text>
            </TouchableOpacity>
            <Text className="text-2xl font-bold text-white">
              Ilmoitukset
            </Text>
          </View>
          <Text className="text-sm text-gray-400 mb-4">
            AI luokitteli ilmoituksesi automaattisesti
          </Text>

          {/* Navigaatiopainikkeet */}
          <View className="flex-row gap-3 mb-6">
            <TouchableOpacity
              onPress={() => router.push("/wellness/calendar")}
              className="flex-1 py-3 rounded-xl items-center"
              style={{ backgroundColor: "#4ecdc420" }}
            >
              <Text style={{ color: "#4ecdc4" }}>📅 Kalenteri</Text>
            </TouchableOpacity>
            <TouchableOpacity
              onPress={() => router.push("/wellness/bills")}
              className="flex-1 py-3 rounded-xl items-center"
              style={{ backgroundColor: "#ff6b6b20" }}
            >
              <Text style={{ color: "#ff6b6b" }}>💰 Laskut</Text>
            </TouchableOpacity>
          </View>

          {isLoading ? (
            <ActivityIndicator color="#4ecdc4" className="mt-8" />
          ) : (
            <>
              {/* Käsittele kaikki */}
              {!allProcessed && (
                <TouchableOpacity
                  onPress={handleProcessAll}
                  className="mb-6 py-3 rounded-xl items-center"
                  style={{ backgroundColor: "#4ecdc4" }}
                >
                  <Text className="font-semibold" style={{ color: "#0a0a0f" }}>
                    ✨ Käsittele kaikki
                  </Text>
                </TouchableOpacity>
              )}

              {allProcessed && (
                <View className="mb-6 py-3 rounded-xl items-center" style={{ backgroundColor: "#1a1a24" }}>
                  <Text className="text-gray-500">
                    ✅ Kaikki ilmoitukset käsitelty
                  </Text>
                </View>
              )}

              <CategorySection
                notifications={notifications ?? []}
                processedIds={processedNotificationIds}
              />
            </>
          )}

          <View style={{ height: 32 }} />
        </ScrollView>
      </SafeAreaView>
    </LinearGradient>
  );
}
