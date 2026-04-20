import { writable, derived, type Readable } from 'svelte/store';

export type IntensityPreference = 'light' | 'medium' | 'hard';

export interface UserSettings {
	onboardingDone: boolean;
	wakeUpTime: string; // "HH:MM"
	sleepHours: number;
	intensityPreference: IntensityPreference;
	selectedSymptoms: string[];
}

export const defaultSettings: UserSettings = {
	onboardingDone: false,
	wakeUpTime: '07:00',
	sleepHours: 8,
	intensityPreference: 'medium',
	selectedSymptoms: []
};

export const settings = writable<UserSettings>(defaultSettings);

export const bedtimeDeadline: Readable<Date> = derived(settings, ($s) => {
	const [h, m] = $s.wakeUpTime.split(':').map(Number);
	const now = new Date();
	const wake = new Date(now);
	wake.setHours(h, m, 0, 0);

	if (wake <= now) wake.setDate(wake.getDate() + 1);

	const deadline = new Date(wake.getTime() - $s.sleepHours * 60 * 60 * 1000);
	return deadline;
});
