import { View, Text, ScrollView, TouchableOpacity, ActivityIndicator } from "react-native";
import { useRouter } from "expo-router";
import { LinearGradient } from "expo-linear-gradient";
import { SafeAreaView } from "react-native-safe-area-context";
import { useNotificationsByCategory } from "@/hooks/useNotifications";

export default function CalendarScreen() {
  const router = useRouter();
  const { data: events, isLoading } = useNotificationsByCategory("calendar");

  return (
    <LinearGradient colors={["#0a0a0f", "#111118", "#0a0a0f"]} style={{ flex: 1 }}>
      <SafeAreaView style={{ flex: 1 }}>
        <ScrollView className="flex-1 px-4 pt-4">
          <View className="flex-row items-center mb-2">
            <TouchableOpacity onPress={() => router.back()} className="mr-3">
              <Text className="text-2xl text-white">←</Text>
            </TouchableOpacity>
            <Text className="text-2xl font-bold" style={{ color: "#4ecdc4" }}>
              📅 Kalenteri
            </Text>
          </View>
          <Text className="text-sm text-gray-400 mb-6">
            Ilmoituksista poimitut tapahtumat
          </Text>

          {isLoading ? (
            <ActivityIndicator color="#4ecdc4" className="mt-8" />
          ) : (
            events?.map((event) => (
              <View
                key={event.id}
                className="rounded-xl p-4 mb-3"
                style={{ backgroundColor: "#1a1a24", borderLeftWidth: 3, borderLeftColor: "#4ecdc4" }}
              >
                <Text className="text-base font-semibold text-white">
                  {event.title}
                </Text>
                <Text className="text-sm text-gray-400 mt-1">
                  {event.body}
                </Text>
                {event.eventDate && (
                  <Text className="text-xs mt-2" style={{ color: "#4ecdc4" }}>
                    {new Date(event.eventDate).toLocaleDateString("fi-FI", {
                      weekday: "long",
                      day: "numeric",
                      month: "long",
                      hour: "2-digit",
                      minute: "2-digit",
                    })}
                  </Text>
                )}
                <Text className="text-xs text-gray-500 mt-1">
                  {event.source}
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
