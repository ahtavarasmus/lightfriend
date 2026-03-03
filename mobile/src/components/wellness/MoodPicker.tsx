import { View, Text, TouchableOpacity } from "react-native";
import type { MoodEmoji } from "@/types/wellness";

const MOODS: { emoji: MoodEmoji; label: string }[] = [
  { emoji: "😊", label: "Hyvä" },
  { emoji: "😐", label: "Ok" },
  { emoji: "😔", label: "Surullinen" },
  { emoji: "😤", label: "Stressaa" },
  { emoji: "😴", label: "Väsynyt" },
];

interface Props {
  selected: MoodEmoji | null;
  onSelect: (mood: MoodEmoji) => void;
}

export default function MoodPicker({ selected, onSelect }: Props) {
  return (
    <View className="mb-6">
      <Text className="text-base font-semibold text-white mb-3">
        Miltä tuntuu tänään?
      </Text>
      <View className="flex-row justify-between">
        {MOODS.map(({ emoji, label }) => (
          <TouchableOpacity
            key={emoji}
            onPress={() => onSelect(emoji)}
            className="items-center px-2 py-3 rounded-xl"
            style={{
              backgroundColor: selected === emoji ? "#4ecdc420" : "#1a1a24",
              borderWidth: selected === emoji ? 2 : 0,
              borderColor: "#4ecdc4",
              minWidth: 60,
            }}
          >
            <Text style={{ fontSize: 28 }}>{emoji}</Text>
            <Text className="text-xs text-gray-400 mt-1">{label}</Text>
          </TouchableOpacity>
        ))}
      </View>
    </View>
  );
}
