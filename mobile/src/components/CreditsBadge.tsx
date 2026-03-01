import { View, Text } from "react-native";

interface Props {
  credits: number;
}

export function CreditsBadge({ credits }: Props) {
  return (
    <View className="flex-row items-center rounded-full bg-primary/10 px-3 py-1">
      <Text className="text-sm font-semibold text-primary">
        {credits} credit{credits !== 1 ? "s" : ""}
      </Text>
    </View>
  );
}
