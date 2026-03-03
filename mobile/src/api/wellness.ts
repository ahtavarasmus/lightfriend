import type {
  CheckIn,
  MoodEmoji,
  WellnessPoints,
  WellnessStats,
} from "@/types/wellness";

// Mock check-in historia
const MOCK_CHECKINS: CheckIn[] = [
  { id: "ci-1", date: "2026-02-28", mood: "😊", energy: 4, sleep: 4, createdAt: new Date("2026-02-28T08:00:00") },
  { id: "ci-2", date: "2026-02-27", mood: "😐", energy: 3, sleep: 3, createdAt: new Date("2026-02-27T09:15:00") },
  { id: "ci-3", date: "2026-02-26", mood: "😊", energy: 5, sleep: 5, createdAt: new Date("2026-02-26T07:30:00") },
  { id: "ci-4", date: "2026-02-25", mood: "😔", energy: 2, sleep: 2, createdAt: new Date("2026-02-25T10:00:00") },
  { id: "ci-5", date: "2026-02-24", mood: "😤", energy: 3, sleep: 1, createdAt: new Date("2026-02-24T08:45:00") },
  { id: "ci-6", date: "2026-02-23", mood: "😊", energy: 4, sleep: 4, createdAt: new Date("2026-02-23T07:00:00") },
  { id: "ci-7", date: "2026-02-22", mood: "😴", energy: 1, sleep: 5, createdAt: new Date("2026-02-22T11:00:00") },
];

const MOCK_POINTS: WellnessPoints = {
  totalPoints: 1250,
  currentStreak: 5,
  longestStreak: 14,
  level: 3,
  achievements: [
    {
      id: "ach-1",
      title: "Ensimmäinen check-in",
      description: "Teit ensimmäisen päivittäisen check-inin",
      emoji: "🌱",
      unlockedAt: new Date("2026-02-15"),
    },
    {
      id: "ach-2",
      title: "Viikon putki",
      description: "7 peräkkäistä check-iniä",
      emoji: "🔥",
      unlockedAt: new Date("2026-02-22"),
    },
    {
      id: "ach-3",
      title: "Ilmoitusten mestari",
      description: "Käsittelit 100 ilmoitusta",
      emoji: "📬",
      unlockedAt: new Date("2026-02-25"),
    },
    {
      id: "ach-4",
      title: "Dumbphone-päivä",
      description: "Käytit dumbphone-tilaa kokonaisen päivän",
      emoji: "📵",
      unlockedAt: null,
    },
    {
      id: "ach-5",
      title: "Kuukauden putki",
      description: "30 peräkkäistä check-iniä",
      emoji: "💎",
      unlockedAt: null,
    },
  ],
};

const MOCK_STATS: WellnessStats = {
  screenTime: {
    label: "Ruutuaika",
    before: 6.5,
    after: 3.2,
    unit: "h/pv",
    improvement: 51,
  },
  notifications: {
    label: "Ilmoitukset",
    before: 147,
    after: 23,
    unit: "kpl/pv",
    improvement: 84,
  },
  focusTime: {
    label: "Fokusaika",
    before: 1.5,
    after: 4.2,
    unit: "h/pv",
    improvement: 180,
  },
  pickups: {
    label: "Puhelimen nostot",
    before: 89,
    after: 31,
    unit: "kpl/pv",
    improvement: 65,
  },
};

export async function getCheckIns(): Promise<CheckIn[]> {
  await new Promise((r) => setTimeout(r, 300));
  return MOCK_CHECKINS;
}

export async function submitCheckIn(data: {
  mood: MoodEmoji;
  energy: number;
  sleep: number;
}): Promise<CheckIn> {
  await new Promise((r) => setTimeout(r, 500));
  const today = new Date().toISOString().split("T")[0];
  return {
    id: `ci-${Date.now()}`,
    date: today,
    mood: data.mood,
    energy: data.energy,
    sleep: data.sleep,
    createdAt: new Date(),
  };
}

export async function getPoints(): Promise<WellnessPoints> {
  await new Promise((r) => setTimeout(r, 300));
  return MOCK_POINTS;
}

export async function getStats(): Promise<WellnessStats> {
  await new Promise((r) => setTimeout(r, 300));
  return MOCK_STATS;
}
