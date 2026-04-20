import { getRecentInteractions } from '$lib/core/storage';

export interface Interaction {
	type: 'clock' | 'math' | 'acknowledge' | 'breathe';
	title: string;
	instruction: string;
	timeLimit: number; // seconds
}

const interactions: Interaction[] = [
	{
		type: 'clock',
		title: 'Kellonaika',
		instruction: 'Kirjoita nykyinen kellonaika',
		timeLimit: 15
	},
	{
		type: 'math',
		title: 'Laskutehtävä',
		instruction: '',  // generated dynamically
		timeLimit: 20
	},
	{
		type: 'acknowledge',
		title: 'Tunnustus',
		instruction: 'Kirjoita "Menen nukkumaan"',
		timeLimit: 10
	},
	{
		type: 'breathe',
		title: 'Hengitys',
		instruction: 'Hengitä sisään... ja ulos...',
		timeLimit: 15
	}
];

export function generateMathProblem(): { question: string; answer: number } {
	const a = Math.floor(Math.random() * 20) + 5;
	const b = Math.floor(Math.random() * 20) + 5;
	const ops = ['+', '-', '*'] as const;
	const op = ops[Math.floor(Math.random() * ops.length)];
	let answer: number;
	switch (op) {
		case '+': answer = a + b; break;
		case '-': answer = a - b; break;
		case '*': answer = a * b; break;
	}
	return { question: `${a} ${op} ${b} = ?`, answer };
}

export async function pickInteraction(): Promise<Interaction> {
	const recent = await getRecentInteractions(5);
	const recentTypes = recent.map((r) => r.type);

	const weights = interactions.map((i) => {
		const count = recentTypes.filter((t) => t === i.type).length;
		return Math.max(1, 10 - count * 3);
	});

	const total = weights.reduce((a, b) => a + b, 0);
	let rand = Math.random() * total;

	for (let idx = 0; idx < interactions.length; idx++) {
		rand -= weights[idx];
		if (rand <= 0) return { ...interactions[idx] };
	}

	return { ...interactions[0] };
}
