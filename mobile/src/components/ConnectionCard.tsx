import { View, Text, Pressable } from "react-native";
import type { ConnectionStatus } from "@/types/api";

interface Props {
  service: string;
  status: ConnectionStatus;
  onPress: () => void;
}

const SERVICE_LABELS: Record<string, string> = {
  whatsapp: "WhatsApp",
  signal: "Signal",
  telegram: "Telegram",
  email: "Email",
  google_calendar: "Google Calendar",
  tesla: "Tesla",
  youtube: "YouTube",
  uber: "Uber",
};

export function ConnectionCard({ service, status, onPress }: Props) {
  return (
    <Pressable
      onPress={onPress}
      className="mb-3 flex-row items-center justify-between rounded-xl bg-white p-4 shadow-sm"
    >
      <View className="flex-1">
        <Text className="text-base font-semibold text-gray-900">
          {SERVICE_LABELS[service] ?? service}
        </Text>
        {status.username && (
          <Text className="mt-0.5 text-sm text-gray-500">
            {status.username}
          </Text>
        )}
      </View>
      <View
        className={`rounded-full px-3 py-1 ${
          status.connected ? "bg-green-100" : "bg-gray-100"
        }`}
      >
        <Text
          className={`text-sm font-medium ${
            status.connected ? "text-green-700" : "text-gray-500"
          }`}
        >
          {status.connected ? "Connected" : "Connect"}
        </Text>
      </View>
    </Pressable>
  );
}
