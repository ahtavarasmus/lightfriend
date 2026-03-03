import { View, Text, ScrollView, TouchableOpacity, ActivityIndicator } from "react-native";
import { useRouter } from "expo-router";
import { LinearGradient } from "expo-linear-gradient";
import { SafeAreaView } from "react-native-safe-area-context";
import { useStats } from "@/hooks/useWellness";
import StatCard from "@/components/wellness/StatCard";

export default function StatsScreen() {
  const router = useRouter();
  const { data: stats, isLoading } = useStats();

  return (
    <LinearGradient colors={["#0a0a0f", "#111118", "#0a0a0f"]} style={{ flex: 1 }}>
      <SafeAreaView style={{ flex: 1 }}>
        <ScrollView className="flex-1 px-4 pt-4">
          <View className="flex-row items-center mb-2">
            <TouchableOpacity onPress={() => router.back()} className="mr-3">
              <Text className="text-2xl text-white">←</Text>
            </TouchableOpacity>
            <Text className="text-2xl font-bold text-white">
              📊 Tilastot
            </Text>
          </View>
          <Text className="text-sm text-gray-400 mb-6">
            Ennen ja jälkeen Lightfriendin käytön
          </Text>

          {isLoading ? (
            <ActivityIndicator color="#4ecdc4" className="mt-8" />
          ) : stats ? (
            <>
              {/* Yhteenveto */}
              <View
                className="rounded-2xl p-5 mb-6 items-center"
                style={{ backgroundColor: "#4ecdc410" }}
              >
                <Text style={{ fontSize: 32 }}>🎉</Text>
                <Text className="text-lg font-bold text-white mt-2">
                  Puhelimesi on kevyempi!
                </Text>
                <Text className="text-sm text-gray-400 mt-1 text-center">
                  Olet vähentänyt häiriöitä merkittävästi
                </Text>
              </View>

              <StatCard stat={stats.screenTime} />
              <StatCard stat={stats.notifications} />
              <StatCard stat={stats.focusTime} />
              <StatCard stat={stats.pickups} />
            </>
          ) : null}

          <View style={{ height: 32 }} />
        </ScrollView>
      </SafeAreaView>
    </LinearGradient>
  );
}
