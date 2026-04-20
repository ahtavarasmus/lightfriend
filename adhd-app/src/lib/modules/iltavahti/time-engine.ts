import type { IntensityLevel } from '$lib/core/events';

export function calculateDeadline(wakeTimeStr: string, sleepHours: number): Date {
	const [h, m] = wakeTimeStr.split(':').map(Number);
	const now = new Date();
	const wake = new Date(now);
	wake.setHours(h, m, 0, 0);

	if (wake <= now) wake.setDate(wake.getDate() + 1);

	return new Date(wake.getTime() - sleepHours * 60 * 60 * 1000);
}

export function sleepIfNow(wakeTimeStr: string): number {
	const [h, m] = wakeTimeStr.split(':').map(Number);
	const now = new Date();
	const wake = new Date(now);
	wake.setHours(h, m, 0, 0);

	if (wake <= now) wake.setDate(wake.getDate() + 1);

	return (wake.getTime() - now.getTime()) / (1000 * 60 * 60);
}

export function hoursUntilDeadline(deadline: Date): number {
	return (deadline.getTime() - Date.now()) / (1000 * 60 * 60);
}

export function getIntensityLevel(hoursRemaining: number): IntensityLevel {
	if (hoursRemaining > 2) return 'calm';
	if (hoursRemaining > 1) return 'gentle';
	if (hoursRemaining > 0.5) return 'warning';
	if (hoursRemaining > 0) return 'urgent';
	return 'overdue';
}

export function getIntensityColor(level: IntensityLevel): string {
	const colors: Record<IntensityLevel, string> = {
		calm: '#7eb2ff',
		gentle: '#ffab40',
		warning: '#ff7043',
		urgent: '#ff5252',
		overdue: '#e040fb'
	};
	return colors[level];
}

export function formatHoursMinutes(hours: number): string {
	if (hours <= 0) return '0:00';
	const h = Math.floor(hours);
	const m = Math.floor((hours - h) * 60);
	return `${h}:${m.toString().padStart(2, '0')}`;
}

export function formatTime(date: Date): string {
	return date.toLocaleTimeString('fi-FI', { hour: '2-digit', minute: '2-digit' });
}
