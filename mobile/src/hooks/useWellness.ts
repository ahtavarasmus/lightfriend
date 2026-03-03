import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import * as wellnessApi from "@/api/wellness";
import type { MoodEmoji } from "@/types/wellness";

export function useCheckIns() {
  return useQuery({
    queryKey: ["wellness-checkins"],
    queryFn: wellnessApi.getCheckIns,
  });
}

export function useSubmitCheckIn() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: { mood: MoodEmoji; energy: number; sleep: number }) =>
      wellnessApi.submitCheckIn(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["wellness-checkins"] });
      queryClient.invalidateQueries({ queryKey: ["wellness-points"] });
    },
  });
}

export function usePoints() {
  return useQuery({
    queryKey: ["wellness-points"],
    queryFn: wellnessApi.getPoints,
  });
}

export function useStats() {
  return useQuery({
    queryKey: ["wellness-stats"],
    queryFn: wellnessApi.getStats,
  });
}
