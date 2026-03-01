import api from "./client";
import type { UserProfile, ProfileUpdateRequest } from "@/types/api";

export async function getProfile(): Promise<UserProfile> {
  const res = await api.get<UserProfile>("/api/profile");
  return res.data;
}

export async function updateProfile(
  data: ProfileUpdateRequest,
): Promise<UserProfile> {
  const res = await api.post<UserProfile>("/api/profile/update", data);
  return res.data;
}
