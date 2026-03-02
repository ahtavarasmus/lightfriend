import { apiRequest } from './client';
import type { TriageItem } from './types';

export async function getTriageItems(): Promise<TriageItem[]> {
  const response = await apiRequest<{ items: TriageItem[] }>('/api/triage');
  return response.items;
}

export async function executeTriageItem(id: number): Promise<void> {
  await apiRequest(`/api/triage/${id}/execute`, { method: 'POST' });
}

export async function snoozeTriageItem(id: number): Promise<void> {
  await apiRequest(`/api/triage/${id}/snooze`, { method: 'POST' });
}

export async function dismissTriageItem(id: number): Promise<void> {
  await apiRequest(`/api/triage/${id}/dismiss`, { method: 'POST' });
}
