import { View, Text, Pressable, Alert } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { router } from "expo-router";
import * as WebBrowser from "expo-web-browser";
import { useConnectionStatus, useDisconnectService } from "@/hooks/useConnection";
import { API_URL } from "@/constants/config";
import { useAuthStore } from "@/stores/authStore";

export default function CalendarScreen() {
  const { data: status } = useConnectionStatus("google_calendar");
  const disconnect = useDisconnectService();
  const accessToken = useAuthStore((s) => s.accessToken);

  const handleConnect = async () => {
    const url = `${API_URL}/api/auth/google_calendar/connect?source=mobile&token=${accessToken}`;
    await WebBrowser.openBrowserAsync(url);
  };

  const handleDisconnect = () => {
    Alert.alert("Disconnect", "Disconnect Google Calendar?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Disconnect",
        style: "destructive",
        onPress: () => disconnect.mutate("google_calendar"),
      },
    ]);
  };

  return (
    <SafeAreaView className="flex-1 bg-gray-50">
      <View className="flex-row items-center border-b border-gray-200 bg-white px-4 py-3">
        <Pressable onPress={() => router.back()} className="mr-3 py-1">
          <Text className="text-primary">← Back</Text>
        </Pressable>
        <Text className="text-xl font-bold text-gray-900">Google Calendar</Text>
      </View>

      <View className="flex-1 px-4 pt-4">
        <View className="rounded-xl bg-white p-4 shadow-sm">
          <Text className="text-base text-gray-600">
            {status?.connected
              ? "Google Calendar is connected"
              : "Connect Google Calendar to let Lightfriend manage your schedule"}
          </Text>
        </View>

        {!status?.connected ? (
          <Pressable onPress={handleConnect} className="mt-4 rounded-xl bg-primary py-4">
            <Text className="text-center text-base font-semibold text-white">
              Connect Google Calendar
            </Text>
          </Pressable>
        ) : (
          <Pressable onPress={handleDisconnect} className="mt-4 rounded-xl bg-red-50 py-4">
            <Text className="text-center text-base font-semibold text-red-600">
              Disconnect
            </Text>
          </Pressable>
        )}
      </View>
    </SafeAreaView>
  );
}
