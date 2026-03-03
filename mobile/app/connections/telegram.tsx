import { View, Text, Pressable, Alert } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { router } from "expo-router";
import * as WebBrowser from "expo-web-browser";
import { useConnectionStatus, useConnectService, useDisconnectService } from "@/hooks/useConnection";

export default function TelegramScreen() {
  const { data: status } = useConnectionStatus("telegram");
  const connect = useConnectService();
  const disconnect = useDisconnectService();

  const handleConnect = async () => {
    const result = await connect.mutateAsync("telegram");
    if ("url" in result) {
      await WebBrowser.openBrowserAsync(result.url);
    }
  };

  const handleDisconnect = () => {
    Alert.alert("Disconnect", "Are you sure you want to disconnect Telegram?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Disconnect",
        style: "destructive",
        onPress: () => disconnect.mutate("telegram"),
      },
    ]);
  };

  return (
    <SafeAreaView className="flex-1 bg-gray-50">
      <View className="flex-row items-center border-b border-gray-200 bg-white px-4 py-3">
        <Pressable onPress={() => router.back()} className="mr-3 py-1">
          <Text className="text-primary">← Back</Text>
        </Pressable>
        <Text className="text-xl font-bold text-gray-900">Telegram</Text>
      </View>

      <View className="flex-1 px-4 pt-4">
        <View className="rounded-xl bg-white p-4 shadow-sm">
          <Text className="text-base text-gray-600">
            {status?.connected
              ? `Connected as ${status.username ?? "Telegram user"}`
              : "Connect Telegram to receive messages through Lightfriend"}
          </Text>
        </View>

        {!status?.connected && (
          <Pressable
            onPress={handleConnect}
            disabled={connect.isPending}
            className="mt-4 rounded-xl bg-primary py-4"
          >
            <Text className="text-center text-base font-semibold text-white">
              {connect.isPending ? "Opening..." : "Connect via Telegram"}
            </Text>
          </Pressable>
        )}

        {status?.connected && (
          <Pressable
            onPress={handleDisconnect}
            disabled={disconnect.isPending}
            className="mt-4 rounded-xl bg-red-50 py-4"
          >
            <Text className="text-center text-base font-semibold text-red-600">
              Disconnect Telegram
            </Text>
          </Pressable>
        )}
      </View>
    </SafeAreaView>
  );
}
