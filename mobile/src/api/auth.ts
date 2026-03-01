import api from "./client";
import type {
  LoginRequest,
  RegisterRequest,
  AuthResponse,
  AuthStatus,
  TotpVerifyRequest,
} from "@/types/api";

export async function login(data: LoginRequest): Promise<AuthResponse> {
  const res = await api.post<AuthResponse>("/api/login", data);
  return res.data;
}

export async function register(data: RegisterRequest): Promise<AuthResponse> {
  const res = await api.post<AuthResponse>("/api/register", data);
  return res.data;
}

export async function refreshTokens(
  refreshToken: string,
): Promise<AuthResponse> {
  const res = await api.post<AuthResponse>("/api/auth/refresh", {
    refresh_token: refreshToken,
  });
  return res.data;
}

export async function getAuthStatus(): Promise<AuthStatus> {
  const res = await api.get<AuthStatus>("/api/auth/status");
  return res.data;
}

export async function verifyTotp(data: TotpVerifyRequest): Promise<AuthResponse> {
  const res = await api.post<AuthResponse>("/api/totp/verify", data);
  return res.data;
}

export async function requestPasswordReset(email: string): Promise<void> {
  await api.post("/api/auth/request-password-reset", { email });
}

export async function logout(): Promise<void> {
  await api.post("/api/logout");
}
