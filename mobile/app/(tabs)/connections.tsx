import { View, Text, ScrollView, ActivityIndicator } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { router } from "expo-router";
import { ConnectionCard } from "@/components/ConnectionCard";
import { useAllConnectionStatuses } from "@/hooks/useConnection";
import type { ServiceName } from "@/api/connections";

const SERVICE_ORDER: ServiceName[] = [
  "whatsapp",
  "signal",
  "telegram",
  "email",
  "google_calendar",
  "tesla",
  "youtube",
  "uber",
];

export default function ConnectionsScreen() {
  const { data, isLoading } = useAllConnectionStatuses();

  return (
    <SafeAreaView className="flex-1 bg-gray-50" edges={["top"]}>
      <View className="border-b border-gray-200 bg-white px-4 py-3">
        <Text className="text-xl font-bold text-gray-900">Connections</Text>
        <Text className="mt-1 text-sm text-gray-500">
          Manage your integrations
        </Text>
      </View>

      {isLoading ? (
        <View className="flex-1 items-center justify-center">
          <ActivityIndicator size="large" color="#6366f1" />
        </View>
      ) : (
        <ScrollView className="flex-1 px-4 pt-4">
          {SERVICE_ORDER.map((service) => {
            const status = data?.[service] ?? {
              connected: false,
              service,
            };
            return (
              <ConnectionCard
                key={service}
                service={service}
                status={status}
                onPress={() => router.push(`/connections/${service}`)}
              />
            );
          })}
        </ScrollView>
      )}
    </SafeAreaView>
  );
}
