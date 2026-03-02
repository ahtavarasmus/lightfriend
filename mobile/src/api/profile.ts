import { apiRequest } from './client';
import type { UserProfile, PushTokenRequest } from './types';

export async function getProfile(): Promise<UserProfile> {
  return apiRequest<UserProfile>('/api/profile');
}

export async function registerPushToken(params: PushTokenRequest): Promise<void> {
  await apiRequest('/api/profile/push-token', {
    method: 'POST',
    body: JSON.stringify(params),
  });
}
