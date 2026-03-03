import { View, Text, TextInput, Pressable, Alert, ScrollView } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { useProfile, useUpdateProfile } from "@/hooks/useProfile";
import { useLogout } from "@/hooks/useAuth";
import { useState, useEffect } from "react";

export default function SettingsScreen() {
  const { data: profile, isLoading } = useProfile();
  const updateProfile = useUpdateProfile();
  const logout = useLogout();

  const [timezone, setTimezone] = useState("");
  const [language, setLanguage] = useState("");
  const [quietStart, setQuietStart] = useState("");
  const [quietEnd, setQuietEnd] = useState("");

  useEffect(() => {
    if (profile) {
      setTimezone(profile.timezone ?? "");
      setLanguage(profile.language ?? "");
      setQuietStart(profile.quiet_hours_start ?? "");
      setQuietEnd(profile.quiet_hours_end ?? "");
    }
  }, [profile]);

  const handleSave = () => {
    updateProfile.mutate(
      {
        timezone: timezone || undefined,
        language: language || undefined,
        quiet_hours_start: quietStart || undefined,
        quiet_hours_end: quietEnd || undefined,
      },
      {
        onSuccess: () => Alert.alert("Saved", "Profile updated successfully"),
        onError: () => Alert.alert("Error", "Failed to update profile"),
      },
    );
  };

  const handleLogout = () => {
    Alert.alert("Logout", "Are you sure you want to sign out?", [
      { text: "Cancel", style: "cancel" },
      {
        text: "Sign Out",
        style: "destructive",
        onPress: () => logout.mutate(),
      },
    ]);
  };

  return (
    <SafeAreaView className="flex-1 bg-gray-50" edges={["top"]}>
      <View className="border-b border-gray-200 bg-white px-4 py-3">
        <Text className="text-xl font-bold text-gray-900">Settings</Text>
      </View>

      <ScrollView className="flex-1 px-4 pt-4">
        {/* Profile Info */}
        <View className="mb-4 rounded-xl bg-white p-4 shadow-sm">
          <Text className="mb-3 text-lg font-semibold text-gray-900">
            Profile
          </Text>
          <Text className="text-sm text-gray-500">
            {isLoading ? "Loading..." : profile?.email}
          </Text>
          <Text className="mt-1 text-sm text-gray-500">
            {profile?.phone_number}
          </Text>
        </View>

        {/* Settings Fields */}
        <View className="mb-4 rounded-xl bg-white p-4 shadow-sm">
          <Text className="mb-3 text-lg font-semibold text-gray-900">
            Preferences
          </Text>

          <Text className="mb-1 text-sm font-medium text-gray-700">
            Timezone
          </Text>
          <TextInput
            className="mb-3 rounded-lg border border-gray-300 px-3 py-2 text-base"
            placeholder="e.g. Europe/Helsinki"
            value={timezone}
            onChangeText={setTimezone}
          />

          <Text className="mb-1 text-sm font-medium text-gray-700">
            Language
          </Text>
          <TextInput
            className="mb-3 rounded-lg border border-gray-300 px-3 py-2 text-base"
            placeholder="e.g. en"
            value={language}
            onChangeText={setLanguage}
          />

          <Text className="mb-1 text-sm font-medium text-gray-700">
            Quiet Hours
          </Text>
          <View className="flex-row gap-3">
            <TextInput
              className="flex-1 rounded-lg border border-gray-300 px-3 py-2 text-base"
              placeholder="22:00"
              value={quietStart}
              onChangeText={setQuietStart}
            />
            <TextInput
              className="flex-1 rounded-lg border border-gray-300 px-3 py-2 text-base"
              placeholder="07:00"
              value={quietEnd}
              onChangeText={setQuietEnd}
            />
          </View>

          <Pressable
            onPress={handleSave}
            disabled={updateProfile.isPending}
            className="mt-4 rounded-xl bg-primary py-3"
          >
            <Text className="text-center text-base font-semibold text-white">
              {updateProfile.isPending ? "Saving..." : "Save Changes"}
            </Text>
          </Pressable>
        </View>

        {/* Logout */}
        <Pressable
          onPress={handleLogout}
          className="mb-8 rounded-xl bg-red-50 py-4"
        >
          <Text className="text-center text-base font-semibold text-red-600">
            Sign Out
          </Text>
        </Pressable>
      </ScrollView>
    </SafeAreaView>
  );
}
