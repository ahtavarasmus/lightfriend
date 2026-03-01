import { useMutation } from "@tanstack/react-query";
import { useAuthStore } from "@/stores/authStore";
import * as authApi from "@/api/auth";
import type { LoginRequest, RegisterRequest, TotpVerifyRequest } from "@/types/api";

export function useLogin() {
  const { setTokens, setUserId, setNeedsTotp } = useAuthStore();

  return useMutation({
    mutationFn: (data: LoginRequest) => authApi.login(data),
    onSuccess: async (res) => {
      // If 2FA is required, the backend returns a specific marker
      if ((res as unknown as { requires_totp: boolean }).requires_totp) {
        setNeedsTotp((res as unknown as { user_id: number }).user_id);
        return;
      }
      await setTokens(res.token, res.refresh_token);
      // Fetch auth status to get user_id
      const status = await authApi.getAuthStatus();
      setUserId(status.user_id);
    },
  });
}

export function useRegister() {
  const { setTokens, setUserId } = useAuthStore();

  return useMutation({
    mutationFn: (data: RegisterRequest) => authApi.register(data),
    onSuccess: async (res) => {
      await setTokens(res.token, res.refresh_token);
      const status = await authApi.getAuthStatus();
      setUserId(status.user_id);
    },
  });
}

export function useVerifyTotp() {
  const { setTokens, setUserId } = useAuthStore();

  return useMutation({
    mutationFn: (data: TotpVerifyRequest) => authApi.verifyTotp(data),
    onSuccess: async (res) => {
      await setTokens(res.token, res.refresh_token);
      const status = await authApi.getAuthStatus();
      setUserId(status.user_id);
    },
  });
}

export function useLogout() {
  const { logout } = useAuthStore();

  return useMutation({
    mutationFn: () => authApi.logout(),
    onSettled: () => {
      logout();
    },
  });
}

export function useRequestPasswordReset() {
  return useMutation({
    mutationFn: (email: string) => authApi.requestPasswordReset(email),
  });
}
