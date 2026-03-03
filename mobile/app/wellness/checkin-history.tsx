import { View, Text, ScrollView, TouchableOpacity, ActivityIndicator } from "react-native";
import { useRouter } from "expo-router";
import { LinearGradient } from "expo-linear-gradient";
import { SafeAreaView } from "react-native-safe-area-context";
import { useCheckIns } from "@/hooks/useWellness";

export default function CheckInHistoryScreen() {
  const router = useRouter();
  const { data: checkIns, isLoading } = useCheckIns();

  function getEnergyBar(value: number) {
    return "█".repeat(value) + "░".repeat(5 - value);
  }

  return (
    <LinearGradient colors={["#0a0a0f", "#111118", "#0a0a0f"]} style={{ flex: 1 }}>
      <SafeAreaView style={{ flex: 1 }}>
        <ScrollView className="flex-1 px-4 pt-4">
          <View className="flex-row items-center mb-2">
            <TouchableOpacity onPress={() => router.back()} className="mr-3">
              <Text className="text-2xl text-white">←</Text>
            </TouchableOpacity>
            <Text className="text-2xl font-bold text-white">
              Check-in historia
            </Text>
          </View>
          <Text className="text-sm text-gray-400 mb-6">
            Viimeiset 7 päivää
          </Text>

          {isLoading ? (
            <ActivityIndicator color="#4ecdc4" className="mt-8" />
          ) : (
            <>
              {/* Fiilismittari-rivi */}
              <View
                className="rounded-2xl p-4 mb-6"
                style={{ backgroundColor: "#1a1a24" }}
              >
                <Text className="text-sm text-gray-400 mb-3">Fiilis-trendi</Text>
                <View className="flex-row justify-between">
                  {checkIns
                    ?.slice(0, 7)
                    .reverse()
                    .map((ci) => (
                      <View key={ci.id} className="items-center">
                        <Text style={{ fontSize: 22 }}>{ci.mood}</Text>
                        <Text className="text-xs text-gray-500 mt-1">
                          {new Date(ci.createdAt).toLocaleDateString("fi-FI", {
                            weekday: "short",
                          })}
                        </Text>
                      </View>
                    ))}
                </View>
              </View>

              {/* Yksityiskohtainen lista */}
              {checkIns?.map((ci) => (
                <View
                  key={ci.id}
                  className="rounded-xl p-4 mb-3"
                  style={{ backgroundColor: "#1a1a24" }}
                >
                  <View className="flex-row items-center justify-between mb-2">
                    <Text className="text-base font-semibold text-white">
                      {new Date(ci.createdAt).toLocaleDateString("fi-FI", {
                        weekday: "long",
                        day: "numeric",
                        month: "long",
                      })}
                    </Text>
                    <Text style={{ fontSize: 24 }}>{ci.mood}</Text>
                  </View>

                  <View className="flex-row justify-between">
                    <View>
                      <Text className="text-xs text-gray-500">Energia</Text>
                      <Text className="text-sm" style={{ color: "#4ecdc4", fontFamily: "monospace" }}>
                        {getEnergyBar(ci.energy)} {ci.energy}/5
                      </Text>
                    </View>
                    <View>
                      <Text className="text-xs text-gray-500">Uni</Text>
                      <Text className="text-sm" style={{ color: "#a78bfa", fontFamily: "monospace" }}>
                        {getEnergyBar(ci.sleep)} {ci.sleep}/5
                      </Text>
                    </View>
                  </View>
                </View>
              ))}
            </>
          )}

          <View style={{ height: 32 }} />
        </ScrollView>
      </SafeAreaView>
    </LinearGradient>
  );
}
