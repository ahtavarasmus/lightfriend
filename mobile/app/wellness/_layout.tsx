import { Stack } from "expo-router";

export default function WellnessLayout() {
  return (
    <Stack screenOptions={{ headerShown: false }}>
      <Stack.Screen name="notifications" />
      <Stack.Screen name="calendar" />
      <Stack.Screen name="bills" />
      <Stack.Screen name="checkin" />
      <Stack.Screen name="checkin-history" />
      <Stack.Screen name="points" />
      <Stack.Screen name="stats" />
    </Stack>
  );
}
