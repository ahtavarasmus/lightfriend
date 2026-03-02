import { setAccessToken, setRefreshToken, clearTokens } from '../utils/storage';
import type { LoginRequest, RegisterRequest, AuthResponse, AuthStatus } from './types';

// Use BASE_URL directly to avoid circular dependency with client.ts token logic
const BASE_URL = __DEV__
  ? 'http://localhost:3000'
  : 'https://app.lightfriend.com';

export async function login(params: LoginRequest): Promise<AuthResponse> {
  const response = await fetch(`${BASE_URL}/api/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(params),
  });

  if (!response.ok) {
    const errorBody = await response.json().catch(() => ({}));
    throw new Error(errorBody.error || 'Login failed');
  }

  const data: AuthResponse = await response.json();
  await setAccessToken(data.token);
  await setRefreshToken(data.refresh_token);
  return data;
}

export async function register(params: RegisterRequest): Promise<AuthResponse> {
  const response = await fetch(`${BASE_URL}/api/register`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(params),
  });

  if (!response.ok) {
    const errorBody = await response.json().catch(() => ({}));
    throw new Error(errorBody.error || 'Registration failed');
  }

  const data: AuthResponse = await response.json();
  await setAccessToken(data.token);
  await setRefreshToken(data.refresh_token);
  return data;
}

export async function logout(): Promise<void> {
  await clearTokens();
}

export async function getAuthStatus(): Promise<AuthStatus> {
  const { apiRequest } = await import('./client');
  return apiRequest<AuthStatus>('/api/auth/status');
}
