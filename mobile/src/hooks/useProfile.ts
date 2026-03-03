import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import * as profileApi from "@/api/profile";
import type { ProfileUpdateRequest } from "@/types/api";

export function useProfile() {
  return useQuery({
    queryKey: ["profile"],
    queryFn: profileApi.getProfile,
  });
}

export function useUpdateProfile() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (data: ProfileUpdateRequest) => profileApi.updateProfile(data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["profile"] });
    },
  });
}
