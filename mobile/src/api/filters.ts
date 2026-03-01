import api from "./client";

export interface FilterRule {
  id: number;
  name: string;
  rule_type: string;
  value: string;
  action: string;
  enabled: boolean;
}

export async function getFilters(): Promise<FilterRule[]> {
  const res = await api.get<FilterRule[]>("/api/filters");
  return res.data;
}

export async function createFilter(
  filter: Omit<FilterRule, "id">,
): Promise<FilterRule> {
  const res = await api.post<FilterRule>("/api/filters", filter);
  return res.data;
}

export async function deleteFilter(id: number): Promise<void> {
  await api.delete(`/api/filters/${id}`);
}
