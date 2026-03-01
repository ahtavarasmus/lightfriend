// Ilmoitusten kategoriat
export type NotificationCategory = "bill" | "calendar" | "important" | "muted";

export interface WellnessNotification {
  id: string;
  title: string;
  body: string;
  category: NotificationCategory;
  source: string;
  timestamp: Date;
  amount?: number; // laskuille (euroina)
  eventDate?: Date; // kalenteritapahtumille
}

export interface CategoryConfig {
  key: NotificationCategory;
  label: string;
  emoji: string;
  color: string;
}

// Check-in
export type MoodEmoji = "😊" | "😐" | "😔" | "😤" | "😴";

export interface CheckIn {
  id: string;
  date: string; // YYYY-MM-DD
  mood: MoodEmoji;
  energy: number; // 1-5
  sleep: number; // 1-5
  createdAt: Date;
}

// Pisteet & saavutukset
export interface WellnessPoints {
  totalPoints: number;
  currentStreak: number;
  longestStreak: number;
  level: number;
  achievements: Achievement[];
}

export interface Achievement {
  id: string;
  title: string;
  description: string;
  emoji: string;
  unlockedAt: Date | null;
}

// Tilastot
export interface WellnessStat {
  label: string;
  before: number;
  after: number;
  unit: string;
  improvement: number; // prosentti
}

export interface WellnessStats {
  screenTime: WellnessStat;
  notifications: WellnessStat;
  focusTime: WellnessStat;
  pickups: WellnessStat;
}

// Dumbphone
export interface DumbphoneState {
  isDumbphone: boolean;
  toggleDumbphone: () => void;
  setDumbphone: (value: boolean) => void;
}
