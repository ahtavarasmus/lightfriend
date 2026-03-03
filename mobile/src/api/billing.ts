import api from "./client";
import type { CreditsDashboard } from "@/types/api";

export async function getCreditsDashboard(): Promise<CreditsDashboard> {
  const res = await api.get<CreditsDashboard>("/api/pricing/dashboard-credits");
  return res.data;
}

export async function createCheckoutSession(): Promise<{ url: string }> {
  const res = await api.post<{ url: string }>("/api/stripe/create-checkout", {
    source: "mobile",
  });
  return res.data;
}
