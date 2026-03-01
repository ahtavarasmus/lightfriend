import { useQuery } from "@tanstack/react-query";
import * as notificationsApi from "@/api/notifications";
import type { NotificationCategory } from "@/types/wellness";

export function useNotifications() {
  return useQuery({
    queryKey: ["wellness-notifications"],
    queryFn: notificationsApi.getNotifications,
    refetchInterval: 60_000,
  });
}

export function useNotificationsByCategory(category: NotificationCategory) {
  return useQuery({
    queryKey: ["wellness-notifications", category],
    queryFn: () => notificationsApi.getNotificationsByCategory(category),
    refetchInterval: 60_000,
  });
}

export function useBillsTotal() {
  return useQuery({
    queryKey: ["wellness-bills-total"],
    queryFn: notificationsApi.getBillsTotal,
  });
}
