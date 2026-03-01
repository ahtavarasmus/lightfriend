import type { WellnessNotification, NotificationCategory } from "@/types/wellness";

// Mock-ilmoitusdata (suomeksi)
const MOCK_NOTIFICATIONS: WellnessNotification[] = [
  // Laskut
  {
    id: "bill-1",
    title: "Telia-lasku",
    body: "Puhelinlasku maaliskuu 2026",
    category: "bill",
    source: "Telia",
    timestamp: new Date("2026-03-01T09:00:00"),
    amount: 29.9,
  },
  {
    id: "bill-2",
    title: "Helen-sähkölasku",
    body: "Sähkölasku helmikuu 2026",
    category: "bill",
    source: "Helen",
    timestamp: new Date("2026-02-28T14:30:00"),
    amount: 67.4,
  },
  {
    id: "bill-3",
    title: "Vattenfall-lasku",
    body: "Kaukolämpölasku Q1/2026",
    category: "bill",
    source: "Vattenfall",
    timestamp: new Date("2026-02-25T10:15:00"),
    amount: 87.5,
  },
  // Kalenteritapahtumat
  {
    id: "cal-1",
    title: "Hammaslääkäri",
    body: "Tarkastus klo 10:00, Oral Kamppi",
    category: "calendar",
    source: "Google Calendar",
    timestamp: new Date("2026-03-03T10:00:00"),
    eventDate: new Date("2026-03-03T10:00:00"),
  },
  {
    id: "cal-2",
    title: "Sprint Review",
    body: "Tiimin sprinttikatsaus Teams-palaveri",
    category: "calendar",
    source: "Outlook",
    timestamp: new Date("2026-03-04T14:00:00"),
    eventDate: new Date("2026-03-04T14:00:00"),
  },
  {
    id: "cal-3",
    title: "Tapaaminen kahvilassa",
    body: "Mikon kanssa Cafe Regattassa klo 16",
    category: "calendar",
    source: "Google Calendar",
    timestamp: new Date("2026-03-05T16:00:00"),
    eventDate: new Date("2026-03-05T16:00:00"),
  },
  // Tärkeät
  {
    id: "imp-1",
    title: "Äidin viesti",
    body: "Soita kun ehdit, tärkeää asiaa!",
    category: "important",
    source: "WhatsApp",
    timestamp: new Date("2026-03-01T08:15:00"),
  },
  {
    id: "imp-2",
    title: "Työnantajan viesti",
    body: "Uusi työsopimuksen päivitys allekirjoitettavaksi",
    category: "important",
    source: "Email",
    timestamp: new Date("2026-03-01T07:30:00"),
  },
  // Mykistetyt
  {
    id: "muted-1",
    title: "Instagram",
    body: "uutta_sisaltoa ja 15 muuta julkaisivat tarinan",
    category: "muted",
    source: "Instagram",
    timestamp: new Date("2026-03-01T06:00:00"),
  },
  {
    id: "muted-2",
    title: "TikTok",
    body: "Tsekkaa uudet trendaavat videot!",
    category: "muted",
    source: "TikTok",
    timestamp: new Date("2026-03-01T05:30:00"),
  },
  {
    id: "muted-3",
    title: "Facebook",
    body: "Sinulla on 3 uutta ilmoitusta",
    category: "muted",
    source: "Facebook",
    timestamp: new Date("2026-03-01T05:00:00"),
  },
];

export async function getNotifications(): Promise<WellnessNotification[]> {
  // Simuloidaan API-viive
  await new Promise((r) => setTimeout(r, 300));
  return MOCK_NOTIFICATIONS;
}

export async function getNotificationsByCategory(
  category: NotificationCategory
): Promise<WellnessNotification[]> {
  await new Promise((r) => setTimeout(r, 200));
  return MOCK_NOTIFICATIONS.filter((n) => n.category === category);
}

export async function getBillsTotal(): Promise<number> {
  const bills = MOCK_NOTIFICATIONS.filter((n) => n.category === "bill");
  return bills.reduce((sum, b) => sum + (b.amount ?? 0), 0);
}
