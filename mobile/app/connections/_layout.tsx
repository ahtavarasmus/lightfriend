import { Stack } from "expo-router";

export default function ConnectionsLayout() {
  return (
    <Stack screenOptions={{ headerShown: false }}>
      <Stack.Screen name="whatsapp" />
      <Stack.Screen name="signal" />
      <Stack.Screen name="telegram" />
      <Stack.Screen name="email" />
      <Stack.Screen name="calendar" />
      <Stack.Screen name="tesla" />
      <Stack.Screen name="youtube" />
      <Stack.Screen name="uber" />
      <Stack.Screen name="mcp" />
    </Stack>
  );
}
