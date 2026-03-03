import { View, Text, TouchableOpacity, Modal } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { useDumbphone } from "@/hooks/useDumbphone";
import { useEffect, useState } from "react";

export default function DumbphoneOverlay() {
  const { isDumbphone, setDumbphone } = useDumbphone();
  const [time, setTime] = useState(new Date());

  useEffect(() => {
    if (!isDumbphone) return;
    const interval = setInterval(() => setTime(new Date()), 1000);
    return () => clearInterval(interval);
  }, [isDumbphone]);

  const hours = time.getHours().toString().padStart(2, "0");
  const minutes = time.getMinutes().toString().padStart(2, "0");
  const dateStr = time.toLocaleDateString("fi-FI", {
    weekday: "long",
    day: "numeric",
    month: "long",
  });

  return (
    <Modal
      visible={isDumbphone}
      animationType="fade"
      presentationStyle="fullScreen"
    >
      <View className="flex-1" style={{ backgroundColor: "#000000" }}>
        <SafeAreaView className="flex-1 items-center justify-between py-12">
          {/* Kello */}
          <View className="items-center mt-16">
            <Text
              className="font-light text-white"
              style={{ fontSize: 72, letterSpacing: 4 }}
            >
              {hours}:{minutes}
            </Text>
            <Text className="text-gray-500 text-lg mt-2 capitalize">
              {dateStr}
            </Text>
          </View>

          {/* Keskialue - minimalistinen */}
          <View className="items-center">
            <Text className="text-gray-600 text-sm">
              Dumbphone-tila aktiivinen
            </Text>
          </View>

          {/* Alapainikkeet */}
          <View className="w-full px-8">
            <View className="flex-row justify-center gap-8 mb-8">
              <TouchableOpacity
                className="items-center px-6 py-4 rounded-2xl"
                style={{ backgroundColor: "#111118" }}
              >
                <Text style={{ fontSize: 28 }}>📞</Text>
                <Text className="text-gray-400 text-xs mt-1">Soita</Text>
              </TouchableOpacity>

              <TouchableOpacity
                className="items-center px-6 py-4 rounded-2xl"
                style={{ backgroundColor: "#111118" }}
              >
                <Text style={{ fontSize: 28 }}>💬</Text>
                <Text className="text-gray-400 text-xs mt-1">Viestit</Text>
              </TouchableOpacity>
            </View>

            <TouchableOpacity
              onPress={() => setDumbphone(false)}
              className="items-center py-3"
            >
              <Text className="text-gray-600 text-sm underline">
                Poistu dumbphone-tilasta
              </Text>
            </TouchableOpacity>
          </View>
        </SafeAreaView>
      </View>
    </Modal>
  );
}
