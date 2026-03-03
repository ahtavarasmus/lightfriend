import { View, Text, ScrollView, TouchableOpacity, Alert } from "react-native";
import { useRouter } from "expo-router";
import { LinearGradient } from "expo-linear-gradient";
import { SafeAreaView } from "react-native-safe-area-context";
import { useState } from "react";
import MoodPicker from "@/components/wellness/MoodPicker";
import SliderInput from "@/components/wellness/SliderInput";
import { useSubmitCheckIn } from "@/hooks/useWellness";
import type { MoodEmoji } from "@/types/wellness";

export default function CheckInScreen() {
  const router = useRouter();
  const [mood, setMood] = useState<MoodEmoji | null>(null);
  const [energy, setEnergy] = useState(3);
  const [sleep, setSleep] = useState(3);
  const submitMutation = useSubmitCheckIn();

  async function handleSubmit() {
    if (!mood) {
      Alert.alert("Valitse fiilis", "Valitse miltä sinusta tuntuu tänään.");
      return;
    }
    await submitMutation.mutateAsync({ mood, energy, sleep });
    Alert.alert("Tallennettu!", "Päivän check-in kirjattu.", [
      { text: "OK", onPress: () => router.back() },
    ]);
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
              Päivän check-in
            </Text>
          </View>
          <Text className="text-sm text-gray-400 mb-6">
            Kirjaa miltä tuntuu tänään
          </Text>

          <MoodPicker selected={mood} onSelect={setMood} />

          <SliderInput
            label="Energiataso"
            emoji="⚡"
            value={energy}
            onChange={setEnergy}
          />

          <SliderInput
            label="Unen laatu"
            emoji="😴"
            value={sleep}
            onChange={setSleep}
          />

          {/* Lähetä */}
          <TouchableOpacity
            onPress={handleSubmit}
            disabled={submitMutation.isPending}
            className="mt-4 py-4 rounded-xl items-center"
            style={{
              backgroundColor: mood ? "#4ecdc4" : "#333",
              opacity: submitMutation.isPending ? 0.5 : 1,
            }}
          >
            <Text
              className="text-base font-semibold"
              style={{ color: mood ? "#0a0a0f" : "#666" }}
            >
              {submitMutation.isPending ? "Tallennetaan..." : "Tallenna check-in"}
            </Text>
          </TouchableOpacity>

          {/* Historia-linkki */}
          <TouchableOpacity
            onPress={() => router.push("/wellness/checkin-history")}
            className="mt-4 py-3 items-center"
          >
            <Text style={{ color: "#4ecdc4" }}>
              📊 Katso historia →
            </Text>
          </TouchableOpacity>

          <View style={{ height: 32 }} />
        </ScrollView>
      </SafeAreaView>
    </LinearGradient>
  );
}
