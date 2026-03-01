import { View, Text, ActivityIndicator } from "react-native";
import { Image } from "expo-image";

interface Props {
  base64Data: string | null;
  isLoading: boolean;
  label: string;
}

export function QRCodeDisplay({ base64Data, isLoading, label }: Props) {
  if (isLoading) {
    return (
      <View className="items-center justify-center py-12">
        <ActivityIndicator size="large" color="#6366f1" />
        <Text className="mt-3 text-gray-500">Loading QR code...</Text>
      </View>
    );
  }

  if (!base64Data) {
    return (
      <View className="items-center justify-center py-12">
        <Text className="text-gray-400">No QR code available</Text>
      </View>
    );
  }

  return (
    <View className="items-center py-6">
      <Image
        source={{ uri: `data:image/png;base64,${base64Data}` }}
        style={{ width: 280, height: 280 }}
        contentFit="contain"
      />
      <Text className="mt-4 text-center text-sm text-gray-600">{label}</Text>
    </View>
  );
}
