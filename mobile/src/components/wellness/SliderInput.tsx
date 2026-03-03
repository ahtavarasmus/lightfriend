import { View, Text, TouchableOpacity } from "react-native";

interface Props {
  label: string;
  value: number;
  onChange: (value: number) => void;
  emoji?: string;
}

export default function SliderInput({ label, value, onChange, emoji }: Props) {
  return (
    <View className="mb-5">
      <Text className="text-base font-semibold text-white mb-3">
        {emoji && <Text>{emoji} </Text>}
        {label}
      </Text>
      <View className="flex-row justify-between gap-2">
        {[1, 2, 3, 4, 5].map((n) => (
          <TouchableOpacity
            key={n}
            onPress={() => onChange(n)}
            className="flex-1 items-center py-3 rounded-xl"
            style={{
              backgroundColor: value === n ? "#4ecdc4" : "#1a1a24",
            }}
          >
            <Text
              className="text-base font-bold"
              style={{ color: value === n ? "#0a0a0f" : "#666666" }}
            >
              {n}
            </Text>
          </TouchableOpacity>
        ))}
      </View>
      <View className="flex-row justify-between mt-1 px-1">
        <Text className="text-xs text-gray-600">Heikko</Text>
        <Text className="text-xs text-gray-600">Erinomainen</Text>
      </View>
    </View>
  );
}
