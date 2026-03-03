import api from "./client";
import type { DashboardSummary, DashboardItem } from "@/types/api";

export async function getDashboardSummary(): Promise<DashboardSummary> {
  const res = await api.get<DashboardSummary>("/api/dashboard/summary");
  return res.data;
}

export async function getItems(): Promise<DashboardItem[]> {
  const res = await api.get<DashboardItem[]>("/api/items");
  return res.data;
}
