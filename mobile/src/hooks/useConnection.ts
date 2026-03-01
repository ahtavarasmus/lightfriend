import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import * as connectionsApi from "@/api/connections";
import type { ServiceName } from "@/api/connections";

export function useConnectionStatus(service: ServiceName) {
  return useQuery({
    queryKey: ["connection-status", service],
    queryFn: () => connectionsApi.getConnectionStatus(service),
    refetchInterval: 10_000, // poll while user is on connection screen
  });
}

export function useAllConnectionStatuses() {
  return useQuery({
    queryKey: ["connection-statuses"],
    queryFn: connectionsApi.getAllConnectionStatuses,
  });
}

export function useConnectService() {
  return useMutation({
    mutationFn: (service: ServiceName) => connectionsApi.connectService(service),
  });
}

export function useDisconnectService() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (service: ServiceName) =>
      connectionsApi.disconnectService(service),
    onSuccess: (_data, service) => {
      queryClient.invalidateQueries({
        queryKey: ["connection-status", service],
      });
      queryClient.invalidateQueries({ queryKey: ["connection-statuses"] });
    },
  });
}
