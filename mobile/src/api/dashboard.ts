import { apiRequest } from './client';
import type { DashboardSummary } from './types';

export async function getDashboardSummary(): Promise<DashboardSummary> {
  return apiRequest<DashboardSummary>('/api/dashboard/summary');
}
