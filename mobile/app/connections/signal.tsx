import { View, Text, Pressable, Alert } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { router } from "expo-router";
import { QRCodeDisplay } from "@/components/QRCodeDisplay";
import { useConnectionStatus, useConnectService, useDisconnectService } from "@/hooks/useConnection";

export default function SignalScreen() {
  const { data: status } = useConnectionStatus("signal");
  const connect = useConnectService();
  const disconnect = useDisconnectService();

  const handleConnect = () => {
    connect.mutate("signal");
  };

  const handleDisconnect = () => {
    Alert.alert("Disconnect", "Are you sure you want to disconnect Signal?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Disconnect",
        style: "destructive",
        onPress: () => disconnect.mutate("signal"),
      },
    ]);
  };

  const qrData = connect.data && "qr_code" in connect.data ? connect.data.qr_code : null;

  return (
    <SafeAreaView className="flex-1 bg-gray-50">
      <View className="flex-row items-center border-b border-gray-200 bg-white px-4 py-3">
        <Pressable onPress={() => router.back()} className="mr-3 py-1">
          <Text className="text-primary">← Back</Text>
        </Pressable>
        <Text className="text-xl font-bold text-gray-900">Signal</Text>
      </View>

      <View className="flex-1 px-4 pt-4">
        <View className="rounded-xl bg-white p-4 shadow-sm">
          <Text className="text-base text-gray-600">
            {status?.connected
              ? `Connected as ${status.username ?? "Signal user"}`
              : "Connect your Signal to receive messages through Lightfriend"}
          </Text>
        </View>

        {!status?.connected && (
          <>
            {qrData ? (
              <QRCodeDisplay
                base64Data={qrData}
                isLoading={false}
                label="Scan this QR code with Signal on your phone"
              />
            ) : (
              <Pressable
                onPress={handleConnect}
                disabled={connect.isPending}
                className="mt-4 rounded-xl bg-primary py-4"
              >
                <Text className="text-center text-base font-semibold text-white">
                  {connect.isPending ? "Loading..." : "Generate QR Code"}
                </Text>
              </Pressable>
            )}
          </>
        )}

        {status?.connected && (
          <Pressable
            onPress={handleDisconnect}
            disabled={disconnect.isPending}
            className="mt-4 rounded-xl bg-red-50 py-4"
          >
            <Text className="text-center text-base font-semibold text-red-600">
              Disconnect Signal
            </Text>
          </Pressable>
        )}
      </View>
    </SafeAreaView>
  );
}
