import { getAccessToken, getRefreshToken, setAccessToken, setRefreshToken, clearTokens } from '../utils/storage';

// Change this to your backend URL
const BASE_URL = __DEV__
  ? 'http://localhost:3000'
  : 'https://app.lightfriend.com';

let onUnauthorized: (() => void) | null = null;

export function setOnUnauthorized(callback: () => void) {
  onUnauthorized = callback;
}

async function refreshAccessToken(): Promise<boolean> {
  const refreshToken = await getRefreshToken();
  if (!refreshToken) return false;

  try {
    const response = await fetch(`${BASE_URL}/api/auth/refresh`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ refresh_token: refreshToken }),
    });

    if (!response.ok) return false;

    const data = await response.json();
    if (data.token) {
      await setAccessToken(data.token);
      if (data.refresh_token) {
        await setRefreshToken(data.refresh_token);
      }
      return true;
    }
    return false;
  } catch {
    return false;
  }
}

export async function apiRequest<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const token = await getAccessToken();

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options.headers as Record<string, string>),
  };

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  let response = await fetch(`${BASE_URL}${path}`, {
    ...options,
    headers,
  });

  // If 401, try refresh
  if (response.status === 401 && token) {
    const refreshed = await refreshAccessToken();
    if (refreshed) {
      const newToken = await getAccessToken();
      headers['Authorization'] = `Bearer ${newToken}`;
      response = await fetch(`${BASE_URL}${path}`, {
        ...options,
        headers,
      });
    } else {
      await clearTokens();
      onUnauthorized?.();
      throw new Error('Session expired');
    }
  }

  if (!response.ok) {
    const errorBody = await response.json().catch(() => ({}));
    throw new Error(errorBody.error || `HTTP ${response.status}`);
  }

  return response.json();
}

export async function apiRequestMultipart<T>(
  path: string,
  formData: FormData,
): Promise<T> {
  const token = await getAccessToken();

  const headers: Record<string, string> = {};
  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const response = await fetch(`${BASE_URL}${path}`, {
    method: 'POST',
    headers,
    body: formData,
  });

  if (response.status === 401) {
    const refreshed = await refreshAccessToken();
    if (refreshed) {
      const newToken = await getAccessToken();
      headers['Authorization'] = `Bearer ${newToken}`;
      const retryResponse = await fetch(`${BASE_URL}${path}`, {
        method: 'POST',
        headers,
        body: formData,
      });
      if (!retryResponse.ok) {
        const errorBody = await retryResponse.json().catch(() => ({}));
        throw new Error(errorBody.error || `HTTP ${retryResponse.status}`);
      }
      return retryResponse.json();
    } else {
      await clearTokens();
      onUnauthorized?.();
      throw new Error('Session expired');
    }
  }

  if (!response.ok) {
    const errorBody = await response.json().catch(() => ({}));
    throw new Error(errorBody.error || `HTTP ${response.status}`);
  }

  return response.json();
}

export { BASE_URL };
