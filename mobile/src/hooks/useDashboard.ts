import { useQuery } from "@tanstack/react-query";
import * as dashboardApi from "@/api/dashboard";

export function useDashboardSummary() {
  return useQuery({
    queryKey: ["dashboard-summary"],
    queryFn: dashboardApi.getDashboardSummary,
    refetchInterval: 60_000,
  });
}

export function useItems() {
  return useQuery({
    queryKey: ["items"],
    queryFn: dashboardApi.getItems,
    refetchInterval: 60_000,
  });
}
