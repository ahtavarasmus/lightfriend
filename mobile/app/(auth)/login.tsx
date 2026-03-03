import { useState } from "react";
import {
  View,
  Text,
  TextInput,
  Pressable,
  ActivityIndicator,
  Alert,
  KeyboardAvoidingView,
  Platform,
  ScrollView,
} from "react-native";
import { Link } from "expo-router";
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useLogin, useVerifyTotp } from "@/hooks/useAuth";
import { useAuthStore } from "@/stores/authStore";

const loginSchema = z.object({
  email: z.string().email("Enter a valid email"),
  password: z.string().min(1, "Password is required"),
});

type LoginForm = z.infer<typeof loginSchema>;

export default function LoginScreen() {
  const login = useLogin();
  const verifyTotp = useVerifyTotp();
  const { needsTotp, pendingUserId } = useAuthStore();
  const [totpCode, setTotpCode] = useState("");

  const {
    control,
    handleSubmit,
    formState: { errors },
  } = useForm<LoginForm>({
    resolver: zodResolver(loginSchema),
    defaultValues: { email: "", password: "" },
  });

  const onSubmit = (data: LoginForm) => {
    login.mutate(data, {
      onError: (err) => {
        Alert.alert("Login Failed", (err as Error).message ?? "Unknown error");
      },
    });
  };

  const onTotpSubmit = () => {
    if (!pendingUserId || !totpCode.trim()) return;
    verifyTotp.mutate(
      { user_id: pendingUserId, code: totpCode.trim() },
      {
        onError: (err) => {
          Alert.alert("Verification Failed", (err as Error).message ?? "Invalid code");
        },
      },
    );
  };

  if (needsTotp) {
    return (
      <View className="flex-1 items-center justify-center bg-white px-6">
        <Text className="mb-2 text-2xl font-bold text-gray-900">
          Two-Factor Authentication
        </Text>
        <Text className="mb-8 text-center text-gray-500">
          Enter the code from your authenticator app
        </Text>

        <TextInput
          className="mb-4 w-full rounded-xl border border-gray-300 px-4 py-3 text-center text-2xl tracking-widest"
          placeholder="000000"
          value={totpCode}
          onChangeText={setTotpCode}
          keyboardType="number-pad"
          maxLength={6}
          autoFocus
        />

        <Pressable
          onPress={onTotpSubmit}
          disabled={verifyTotp.isPending || totpCode.length < 6}
          className="w-full rounded-xl bg-primary py-4"
        >
          {verifyTotp.isPending ? (
            <ActivityIndicator color="#fff" />
          ) : (
            <Text className="text-center text-base font-semibold text-white">
              Verify
            </Text>
          )}
        </Pressable>
      </View>
    );
  }

  return (
    <KeyboardAvoidingView
      className="flex-1 bg-white"
      behavior={Platform.OS === "ios" ? "padding" : "height"}
    >
      <ScrollView
        contentContainerStyle={{ flexGrow: 1, justifyContent: "center" }}
        keyboardShouldPersistTaps="handled"
      >
        <View className="px-6">
          <Text className="mb-2 text-3xl font-bold text-gray-900">
            Welcome back
          </Text>
          <Text className="mb-8 text-gray-500">
            Sign in to your Lightfriend account
          </Text>

          <Text className="mb-1 text-sm font-medium text-gray-700">Email</Text>
          <Controller
            control={control}
            name="email"
            render={({ field: { onChange, onBlur, value } }) => (
              <TextInput
                className="mb-1 rounded-xl border border-gray-300 px-4 py-3 text-base"
                placeholder="you@example.com"
                keyboardType="email-address"
                autoCapitalize="none"
                autoComplete="email"
                value={value}
                onBlur={onBlur}
                onChangeText={onChange}
              />
            )}
          />
          {errors.email && (
            <Text className="mb-3 text-sm text-red-500">
              {errors.email.message}
            </Text>
          )}

          <Text className="mb-1 mt-4 text-sm font-medium text-gray-700">
            Password
          </Text>
          <Controller
            control={control}
            name="password"
            render={({ field: { onChange, onBlur, value } }) => (
              <TextInput
                className="mb-1 rounded-xl border border-gray-300 px-4 py-3 text-base"
                placeholder="Your password"
                secureTextEntry
                autoComplete="password"
                value={value}
                onBlur={onBlur}
                onChangeText={onChange}
              />
            )}
          />
          {errors.password && (
            <Text className="mb-3 text-sm text-red-500">
              {errors.password.message}
            </Text>
          )}

          <Pressable
            onPress={handleSubmit(onSubmit)}
            disabled={login.isPending}
            className="mt-6 rounded-xl bg-primary py-4"
          >
            {login.isPending ? (
              <ActivityIndicator color="#fff" />
            ) : (
              <Text className="text-center text-base font-semibold text-white">
                Sign In
              </Text>
            )}
          </Pressable>

          <Link href="/(auth)/password-reset" asChild>
            <Pressable className="mt-4 py-2">
              <Text className="text-center text-sm text-primary">
                Forgot password?
              </Text>
            </Pressable>
          </Link>

          <Link href="/(auth)/register" asChild>
            <Pressable className="mt-2 py-2">
              <Text className="text-center text-sm text-gray-500">
                Don't have an account?{" "}
                <Text className="font-semibold text-primary">Sign up</Text>
              </Text>
            </Pressable>
          </Link>
        </View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}
