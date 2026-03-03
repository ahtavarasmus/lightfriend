import { useDumbphoneStore } from "@/stores/dumbphoneStore";

export function useDumbphone() {
  const isDumbphone = useDumbphoneStore((s) => s.isDumbphone);
  const toggleDumbphone = useDumbphoneStore((s) => s.toggleDumbphone);
  const setDumbphone = useDumbphoneStore((s) => s.setDumbphone);

  return { isDumbphone, toggleDumbphone, setDumbphone };
}
