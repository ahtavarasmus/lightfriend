import { View, Text, ScrollView, TouchableOpacity, ActivityIndicator } from "react-native";
import { useRouter } from "expo-router";
import { LinearGradient } from "expo-linear-gradient";
import { SafeAreaView } from "react-native-safe-area-context";
import { usePoints } from "@/hooks/useWellness";
import StreakBadge from "@/components/wellness/StreakBadge";

export default function PointsScreen() {
  const router = useRouter();
  const { data: points, isLoading } = usePoints();

  return (
    <LinearGradient colors={["#0a0a0f", "#111118", "#0a0a0f"]} style={{ flex: 1 }}>
      <SafeAreaView style={{ flex: 1 }}>
        <ScrollView className="flex-1 px-4 pt-4">
          <View className="flex-row items-center mb-2">
            <TouchableOpacity onPress={() => router.back()} className="mr-3">
              <Text className="text-2xl text-white">←</Text>
            </TouchableOpacity>
            <Text className="text-2xl font-bold text-white">
              🏆 Pisteet
            </Text>
          </View>
          <Text className="text-sm text-gray-400 mb-6">
            Kevyempi puhelin -gamifikaatio
          </Text>

          {isLoading ? (
            <ActivityIndicator color="#4ecdc4" className="mt-8" />
          ) : points ? (
            <>
              {/* Pisteet ja taso */}
              <View
                className="rounded-2xl p-5 mb-4 items-center"
                style={{ backgroundColor: "#1a1a24" }}
              >
                <Text className="text-sm text-gray-400 mb-1">Kokonaispisteet</Text>
                <Text className="text-4xl font-bold" style={{ color: "#4ecdc4" }}>
                  {points.totalPoints}
                </Text>
                <View
                  className="mt-3 px-4 py-1 rounded-full"
                  style={{ backgroundColor: "#4ecdc420" }}
                >
                  <Text className="text-sm font-medium" style={{ color: "#4ecdc4" }}>
                    Taso {points.level}
                  </Text>
                </View>
              </View>

              {/* Streak */}
              <View className="flex-row gap-3 mb-6">
                <View className="flex-1">
                  <StreakBadge streak={points.currentStreak} label="Nykyinen streak" />
                </View>
                <View className="flex-1">
                  <StreakBadge streak={points.longestStreak} label="Pisin streak" />
                </View>
              </View>

              {/* Saavutukset */}
              <Text className="text-lg font-bold text-white mb-3">
                Saavutukset
              </Text>
              {points.achievements.map((ach) => (
                <View
                  key={ach.id}
                  className="rounded-xl p-4 mb-2 flex-row items-center"
                  style={{
                    backgroundColor: "#1a1a24",
                    opacity: ach.unlockedAt ? 1 : 0.4,
                  }}
                >
                  <Text style={{ fontSize: 28 }}>{ach.emoji}</Text>
                  <View className="ml-3 flex-1">
                    <Text className="text-base font-semibold text-white">
                      {ach.title}
                    </Text>
                    <Text className="text-sm text-gray-400">
                      {ach.description}
                    </Text>
                    {ach.unlockedAt && (
                      <Text className="text-xs mt-1" style={{ color: "#4ecdc4" }}>
                        Avattu {new Date(ach.unlockedAt).toLocaleDateString("fi-FI")}
                      </Text>
                    )}
                    {!ach.unlockedAt && (
                      <Text className="text-xs text-gray-500 mt-1">
                        🔒 Lukittu
                      </Text>
                    )}
                  </View>
                </View>
              ))}
            </>
          ) : null}

          <View style={{ height: 32 }} />
        </ScrollView>
      </SafeAreaView>
    </LinearGradient>
  );
}
