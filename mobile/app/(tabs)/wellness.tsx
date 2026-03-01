import { View, Text, ScrollView, TouchableOpacity } from "react-native";
import { useRouter } from "expo-router";
import { LinearGradient } from "expo-linear-gradient";
import { SafeAreaView } from "react-native-safe-area-context";
import { useDumbphone } from "@/hooks/useDumbphone";

interface FeatureCard {
  title: string;
  description: string;
  emoji: string;
  route: string;
  color: string;
}

const FEATURES: FeatureCard[] = [
  {
    title: "Ilmoitusten luokittelu",
    description: "AI ryhmittelee ilmoituksesi: laskut, tapahtumat, tärkeät ja mykistetyt",
    emoji: "🔔",
    route: "/wellness/notifications",
    color: "#4ecdc4",
  },
  {
    title: "Päivän check-in",
    description: "Kirjaa fiilis, energia ja uni — seuraa trendiä",
    emoji: "📝",
    route: "/wellness/checkin",
    color: "#ffd93d",
  },
  {
    title: "Dumbphone-tila",
    description: "Piilota kaikki paitsi puhelut ja viestit",
    emoji: "📵",
    route: "dumbphone-toggle",
    color: "#ff6b6b",
  },
  {
    title: "Kevyempi puhelin -pisteet",
    description: "Streak, saavutukset ja gamifikaatio",
    emoji: "🏆",
    route: "/wellness/points",
    color: "#a78bfa",
  },
  {
    title: "Ennen/jälkeen -tilastot",
    description: "Ruutuaika, ilmoitukset, fokusaika vertailussa",
    emoji: "📊",
    route: "/wellness/stats",
    color: "#34d399",
  },
];

export default function WellnessHub() {
  const router = useRouter();
  const { toggleDumbphone } = useDumbphone();

  function handlePress(card: FeatureCard) {
    if (card.route === "dumbphone-toggle") {
      toggleDumbphone();
    } else {
      router.push(card.route as any);
    }
  }

  return (
    <LinearGradient
      colors={["#0a0a0f", "#111118", "#0a0a0f"]}
      style={{ flex: 1 }}
    >
      <SafeAreaView style={{ flex: 1 }}>
        <ScrollView className="flex-1 px-4 pt-4">
          <Text className="text-3xl font-bold text-white mb-2">
            Hyvinvointi
          </Text>
          <Text className="text-base text-gray-400 mb-6">
            Kevyempi suhde puhelimeesi
          </Text>

          {FEATURES.map((card) => (
            <TouchableOpacity
              key={card.title}
              onPress={() => handlePress(card)}
              activeOpacity={0.7}
              className="mb-4"
            >
              <View
                className="rounded-2xl p-5"
                style={{ backgroundColor: "#111118" }}
              >
                <View className="flex-row items-center mb-3">
                  <Text style={{ fontSize: 28 }}>{card.emoji}</Text>
                  <Text
                    className="text-lg font-semibold ml-3"
                    style={{ color: card.color }}
                  >
                    {card.title}
                  </Text>
                </View>
                <Text className="text-sm text-gray-400 leading-5">
                  {card.description}
                </Text>
                <View className="flex-row items-center mt-3">
                  <Text className="text-xs" style={{ color: card.color }}>
                    {card.route === "dumbphone-toggle" ? "Aktivoi →" : "Avaa →"}
                  </Text>
                </View>
              </View>
            </TouchableOpacity>
          ))}

          <View style={{ height: 32 }} />
        </ScrollView>
      </SafeAreaView>
    </LinearGradient>
  );
}
