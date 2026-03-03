import { View, Text, Pressable, ActivityIndicator, Alert } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import * as WebBrowser from "expo-web-browser";
import { useQuery, useMutation } from "@tanstack/react-query";
import { getCreditsDashboard, createCheckoutSession } from "@/api/billing";

export default function BillingScreen() {
  const { data, isLoading } = useQuery({
    queryKey: ["credits-dashboard"],
    queryFn: getCreditsDashboard,
  });

  const checkout = useMutation({
    mutationFn: createCheckoutSession,
    onSuccess: async ({ url }) => {
      await WebBrowser.openBrowserAsync(url);
    },
    onError: () => {
      Alert.alert("Error", "Failed to open checkout");
    },
  });

  return (
    <SafeAreaView className="flex-1 bg-gray-50" edges={["top"]}>
      <View className="border-b border-gray-200 bg-white px-4 py-3">
        <Text className="text-xl font-bold text-gray-900">Billing</Text>
      </View>

      {isLoading ? (
        <View className="flex-1 items-center justify-center">
          <ActivityIndicator size="large" color="#6366f1" />
        </View>
      ) : (
        <View className="flex-1 px-4 pt-4">
          {/* Credits Card */}
          <View className="mb-4 rounded-xl bg-white p-6 shadow-sm">
            <Text className="text-sm font-medium uppercase text-gray-400">
              Credits Remaining
            </Text>
            <Text className="mt-1 text-4xl font-bold text-gray-900">
              {data?.credits_remaining ?? 0}
            </Text>
            <View className="mt-3 flex-row items-center">
              <View className="mr-2 h-2 flex-1 rounded-full bg-gray-200">
                <View
                  className="h-2 rounded-full bg-primary"
                  style={{
                    width: `${Math.min(
                      100,
                      ((data?.credits_remaining ?? 0) /
                        Math.max(
                          1,
                          (data?.credits_remaining ?? 0) +
                            (data?.credits_used ?? 0),
                        )) *
                        100,
                    )}%`,
                  }}
                />
              </View>
            </View>
            <Text className="mt-2 text-sm text-gray-500">
              {data?.credits_used ?? 0} credits used
            </Text>
          </View>

          {/* Plan Card */}
          <View className="mb-4 rounded-xl bg-white p-6 shadow-sm">
            <Text className="text-sm font-medium uppercase text-gray-400">
              Current Plan
            </Text>
            <Text className="mt-1 text-xl font-bold text-gray-900">
              {data?.plan ?? "Free"}
            </Text>
            {data?.renewal_date && (
              <Text className="mt-1 text-sm text-gray-500">
                Renews {new Date(data.renewal_date).toLocaleDateString()}
              </Text>
            )}
          </View>

          {/* Upgrade Button */}
          <Pressable
            onPress={() => checkout.mutate()}
            disabled={checkout.isPending}
            className="rounded-xl bg-primary py-4"
          >
            {checkout.isPending ? (
              <ActivityIndicator color="#fff" />
            ) : (
              <Text className="text-center text-base font-semibold text-white">
                Upgrade Plan
              </Text>
            )}
          </Pressable>
        </View>
      )}
    </SafeAreaView>
  );
}
