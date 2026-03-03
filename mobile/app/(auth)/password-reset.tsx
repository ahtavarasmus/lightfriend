import {
  View,
  Text,
  TextInput,
  Pressable,
  ActivityIndicator,
  Alert,
  KeyboardAvoidingView,
  Platform,
} from "react-native";
import { router } from "expo-router";
import { useForm, Controller } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { useRequestPasswordReset } from "@/hooks/useAuth";

const schema = z.object({
  email: z.string().email("Enter a valid email"),
});

type Form = z.infer<typeof schema>;

export default function PasswordResetScreen() {
  const resetPassword = useRequestPasswordReset();

  const {
    control,
    handleSubmit,
    formState: { errors },
  } = useForm<Form>({
    resolver: zodResolver(schema),
    defaultValues: { email: "" },
  });

  const onSubmit = (data: Form) => {
    resetPassword.mutate(data.email, {
      onSuccess: () => {
        Alert.alert(
          "Check your email",
          "If an account exists, we sent a password reset link.",
          [{ text: "OK", onPress: () => router.back() }],
        );
      },
      onError: () => {
        Alert.alert(
          "Check your email",
          "If an account exists, we sent a password reset link.",
          [{ text: "OK", onPress: () => router.back() }],
        );
      },
    });
  };

  return (
    <KeyboardAvoidingView
      className="flex-1 items-center justify-center bg-white px-6"
      behavior={Platform.OS === "ios" ? "padding" : "height"}
    >
      <Text className="mb-2 text-2xl font-bold text-gray-900">
        Reset Password
      </Text>
      <Text className="mb-8 text-center text-gray-500">
        Enter your email and we'll send you a reset link
      </Text>

      <Controller
        control={control}
        name="email"
        render={({ field: { onChange, onBlur, value } }) => (
          <TextInput
            className="mb-1 w-full rounded-xl border border-gray-300 px-4 py-3 text-base"
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
        <Text className="mb-3 w-full text-sm text-red-500">
          {errors.email.message}
        </Text>
      )}

      <Pressable
        onPress={handleSubmit(onSubmit)}
        disabled={resetPassword.isPending}
        className="mt-4 w-full rounded-xl bg-primary py-4"
      >
        {resetPassword.isPending ? (
          <ActivityIndicator color="#fff" />
        ) : (
          <Text className="text-center text-base font-semibold text-white">
            Send Reset Link
          </Text>
        )}
      </Pressable>

      <Pressable onPress={() => router.back()} className="mt-4 py-2">
        <Text className="text-sm text-primary">Back to login</Text>
      </Pressable>
    </KeyboardAvoidingView>
  );
}
