import { View, Text, TextInput, Pressable, Alert, ScrollView } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { router } from "expo-router";
import { useState } from "react";
import { useConnectionStatus, useDisconnectService } from "@/hooks/useConnection";
import api from "@/api/client";

export default function EmailScreen() {
  const { data: status } = useConnectionStatus("email");
  const disconnect = useDisconnectService();

  const [imapHost, setImapHost] = useState("");
  const [imapPort, setImapPort] = useState("993");
  const [emailAddress, setEmailAddress] = useState("");
  const [emailPassword, setEmailPassword] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  const handleConnect = async () => {
    if (!imapHost || !emailAddress || !emailPassword) {
      Alert.alert("Error", "Please fill in all fields");
      return;
    }
    setIsSubmitting(true);
    try {
      await api.post("/api/auth/email/connect", {
        imap_host: imapHost,
        imap_port: parseInt(imapPort, 10),
        email: emailAddress,
        password: emailPassword,
      });
      Alert.alert("Success", "Email connected successfully");
    } catch {
      Alert.alert("Error", "Failed to connect email");
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleDisconnect = () => {
    Alert.alert("Disconnect", "Are you sure?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Disconnect",
        style: "destructive",
        onPress: () => disconnect.mutate("email"),
      },
    ]);
  };

  return (
    <SafeAreaView className="flex-1 bg-gray-50">
      <View className="flex-row items-center border-b border-gray-200 bg-white px-4 py-3">
        <Pressable onPress={() => router.back()} className="mr-3 py-1">
          <Text className="text-primary">← Back</Text>
        </Pressable>
        <Text className="text-xl font-bold text-gray-900">Email (IMAP)</Text>
      </View>

      <ScrollView className="flex-1 px-4 pt-4">
        {status?.connected ? (
          <>
            <View className="rounded-xl bg-white p-4 shadow-sm">
              <Text className="text-base text-gray-600">
                Connected as {status.username ?? "email user"}
              </Text>
            </View>
            <Pressable
              onPress={handleDisconnect}
              className="mt-4 rounded-xl bg-red-50 py-4"
            >
              <Text className="text-center text-base font-semibold text-red-600">
                Disconnect Email
              </Text>
            </Pressable>
          </>
        ) : (
          <View className="rounded-xl bg-white p-4 shadow-sm">
            <Text className="mb-4 text-base text-gray-600">
              Connect your email via IMAP to receive messages through Lightfriend
            </Text>

            <Text className="mb-1 text-sm font-medium text-gray-700">
              IMAP Host
            </Text>
            <TextInput
              className="mb-3 rounded-lg border border-gray-300 px-3 py-2 text-base"
              placeholder="imap.gmail.com"
              value={imapHost}
              onChangeText={setImapHost}
              autoCapitalize="none"
            />

            <Text className="mb-1 text-sm font-medium text-gray-700">
              IMAP Port
            </Text>
            <TextInput
              className="mb-3 rounded-lg border border-gray-300 px-3 py-2 text-base"
              placeholder="993"
              value={imapPort}
              onChangeText={setImapPort}
              keyboardType="number-pad"
            />

            <Text className="mb-1 text-sm font-medium text-gray-700">
              Email Address
            </Text>
            <TextInput
              className="mb-3 rounded-lg border border-gray-300 px-3 py-2 text-base"
              placeholder="you@example.com"
              value={emailAddress}
              onChangeText={setEmailAddress}
              keyboardType="email-address"
              autoCapitalize="none"
            />

            <Text className="mb-1 text-sm font-medium text-gray-700">
              Password / App Password
            </Text>
            <TextInput
              className="mb-3 rounded-lg border border-gray-300 px-3 py-2 text-base"
              placeholder="App password"
              value={emailPassword}
              onChangeText={setEmailPassword}
              secureTextEntry
            />

            <Pressable
              onPress={handleConnect}
              disabled={isSubmitting}
              className="mt-2 rounded-xl bg-primary py-4"
            >
              <Text className="text-center text-base font-semibold text-white">
                {isSubmitting ? "Connecting..." : "Connect Email"}
              </Text>
            </Pressable>
          </View>
        )}
      </ScrollView>
    </SafeAreaView>
  );
}
