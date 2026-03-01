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
import { useRegister } from "@/hooks/useAuth";

const registerSchema = z
  .object({
    email: z.string().email("Enter a valid email"),
    phone_number: z.string().min(10, "Enter a valid phone number"),
    password: z.string().min(8, "Password must be at least 8 characters"),
    confirm_password: z.string(),
  })
  .refine((d) => d.password === d.confirm_password, {
    message: "Passwords don't match",
    path: ["confirm_password"],
  });

type RegisterForm = z.infer<typeof registerSchema>;

export default function RegisterScreen() {
  const register = useRegister();

  const {
    control,
    handleSubmit,
    formState: { errors },
  } = useForm<RegisterForm>({
    resolver: zodResolver(registerSchema),
    defaultValues: {
      email: "",
      phone_number: "",
      password: "",
      confirm_password: "",
    },
  });

  const onSubmit = (data: RegisterForm) => {
    register.mutate(
      {
        email: data.email,
        phone_number: data.phone_number,
        password: data.password,
      },
      {
        onError: (err) => {
          Alert.alert(
            "Registration Failed",
            (err as Error).message ?? "Unknown error",
          );
        },
      },
    );
  };

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
            Create account
          </Text>
          <Text className="mb-8 text-gray-500">
            Get started with Lightfriend
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
            <Text className="mb-2 text-sm text-red-500">
              {errors.email.message}
            </Text>
          )}

          <Text className="mb-1 mt-3 text-sm font-medium text-gray-700">
            Phone Number
          </Text>
          <Controller
            control={control}
            name="phone_number"
            render={({ field: { onChange, onBlur, value } }) => (
              <TextInput
                className="mb-1 rounded-xl border border-gray-300 px-4 py-3 text-base"
                placeholder="+1234567890"
                keyboardType="phone-pad"
                autoComplete="tel"
                value={value}
                onBlur={onBlur}
                onChangeText={onChange}
              />
            )}
          />
          {errors.phone_number && (
            <Text className="mb-2 text-sm text-red-500">
              {errors.phone_number.message}
            </Text>
          )}

          <Text className="mb-1 mt-3 text-sm font-medium text-gray-700">
            Password
          </Text>
          <Controller
            control={control}
            name="password"
            render={({ field: { onChange, onBlur, value } }) => (
              <TextInput
                className="mb-1 rounded-xl border border-gray-300 px-4 py-3 text-base"
                placeholder="At least 8 characters"
                secureTextEntry
                autoComplete="new-password"
                value={value}
                onBlur={onBlur}
                onChangeText={onChange}
              />
            )}
          />
          {errors.password && (
            <Text className="mb-2 text-sm text-red-500">
              {errors.password.message}
            </Text>
          )}

          <Text className="mb-1 mt-3 text-sm font-medium text-gray-700">
            Confirm Password
          </Text>
          <Controller
            control={control}
            name="confirm_password"
            render={({ field: { onChange, onBlur, value } }) => (
              <TextInput
                className="mb-1 rounded-xl border border-gray-300 px-4 py-3 text-base"
                placeholder="Repeat password"
                secureTextEntry
                value={value}
                onBlur={onBlur}
                onChangeText={onChange}
              />
            )}
          />
          {errors.confirm_password && (
            <Text className="mb-2 text-sm text-red-500">
              {errors.confirm_password.message}
            </Text>
          )}

          <Pressable
            onPress={handleSubmit(onSubmit)}
            disabled={register.isPending}
            className="mt-6 rounded-xl bg-primary py-4"
          >
            {register.isPending ? (
              <ActivityIndicator color="#fff" />
            ) : (
              <Text className="text-center text-base font-semibold text-white">
                Create Account
              </Text>
            )}
          </Pressable>

          <Link href="/(auth)/login" asChild>
            <Pressable className="mt-4 py-2">
              <Text className="text-center text-sm text-gray-500">
                Already have an account?{" "}
                <Text className="font-semibold text-primary">Sign in</Text>
              </Text>
            </Pressable>
          </Link>
        </View>
      </ScrollView>
    </KeyboardAvoidingView>
  );
}
