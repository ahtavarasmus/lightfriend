import api from "./client";
import type { ConnectionStatus, QRCodeResponse } from "@/types/api";

const SERVICES = [
  "whatsapp",
  "signal",
  "telegram",
  "email",
  "google_calendar",
  "tesla",
  "youtube",
  "uber",
] as const;

export type ServiceName = (typeof SERVICES)[number];

export async function getConnectionStatus(
  service: ServiceName,
): Promise<ConnectionStatus> {
  const res = await api.get<ConnectionStatus>(`/api/auth/${service}/status`);
  return res.data;
}

export async function getAllConnectionStatuses(): Promise<
  Record<ServiceName, ConnectionStatus>
> {
  const results = await Promise.allSettled(
    SERVICES.map((s) => getConnectionStatus(s)),
  );

  const statuses = {} as Record<ServiceName, ConnectionStatus>;
  SERVICES.forEach((service, i) => {
    const result = results[i];
    statuses[service] =
      result.status === "fulfilled"
        ? result.value
        : { connected: false, service };
  });
  return statuses;
}

export async function connectService(
  service: ServiceName,
): Promise<QRCodeResponse | { url: string }> {
  const res = await api.get(`/api/auth/${service}/connect`);
  return res.data;
}

export async function disconnectService(service: ServiceName): Promise<void> {
  await api.post(`/api/auth/${service}/disconnect`);
}
