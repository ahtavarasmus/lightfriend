<script lang="ts">
	import { createEventDispatcher, onMount, onDestroy } from 'svelte';
	import { generateMathProblem, type Interaction } from './interaction-picker';
	import { logInteraction } from '$lib/core/storage';
	import { emit } from '$lib/core/events';

	export let interaction: Interaction;

	const dispatch = createEventDispatcher<{ done: void }>();

	let timeLeft: number;
	let input = '';
	let mathProblem: { question: string; answer: number } | null = null;
	let breathePhase: 'in' | 'out' = 'in';
	let error = '';
	let started = Date.now();
	let timer: ReturnType<typeof setInterval>;

	$: timeLeft = interaction.timeLimit;

	onMount(() => {
		started = Date.now();
		timeLeft = interaction.timeLimit;

		if (interaction.type === 'math') {
			mathProblem = generateMathProblem();
		}

		if (interaction.type === 'breathe') {
			const breatheTimer = setInterval(() => {
				breathePhase = breathePhase === 'in' ? 'out' : 'in';
			}, 4000);
			timer = setInterval(() => {
				timeLeft--;
				if (timeLeft <= 0) {
					clearInterval(breatheTimer);
					complete(true);
				}
			}, 1000);
			return () => clearInterval(breatheTimer);
		}

		timer = setInterval(() => {
			timeLeft--;
			if (timeLeft <= 0) {
				timeout();
			}
		}, 1000);
	});

	onDestroy(() => {
		if (timer) clearInterval(timer);
	});

	function complete(success: boolean) {
		clearInterval(timer);
		const durationMs = Date.now() - started;
		logInteraction({
			type: interaction.type,
			timestamp: Date.now(),
			durationMs,
			completed: success
		});
		if (success) {
			emit('interaction-completed', { type: interaction.type, durationMs });
		}
		dispatch('done');
	}

	function timeout() {
		clearInterval(timer);
		logInteraction({
			type: interaction.type,
			timestamp: Date.now(),
			durationMs: interaction.timeLimit * 1000,
			completed: false
		});
		emit('interaction-timeout', { type: interaction.type });
		dispatch('done');
	}

	function submit() {
		error = '';
		if (interaction.type === 'clock') {
			const now = new Date();
			const currentTime = `${now.getHours().toString().padStart(2, '0')}:${now.getMinutes().toString().padStart(2, '0')}`;
			const cleaned = input.trim().replace('.', ':');
			if (cleaned === currentTime) {
				complete(true);
			} else {
				error = 'Väärä aika, yritä uudelleen';
			}
		} else if (interaction.type === 'math' && mathProblem) {
			if (parseInt(input.trim()) === mathProblem.answer) {
				complete(true);
			} else {
				error = 'Väärä vastaus';
			}
		} else if (interaction.type === 'acknowledge') {
			if (input.trim().toLowerCase() === 'menen nukkumaan') {
				complete(true);
			} else {
				error = 'Kirjoita tarkasti: Menen nukkumaan';
			}
		}
	}
</script>

<div class="modal-backdrop">
	<div class="modal">
		<div class="timer-bar">
			<div class="timer-fill" style="width: {(timeLeft / interaction.timeLimit) * 100}%"></div>
		</div>
		<h3>{interaction.title}</h3>
		<p class="time-left">{timeLeft}s</p>

		{#if interaction.type === 'breathe'}
			<div class="breathe-circle" class:inhale={breathePhase === 'in'}>
				<span>{breathePhase === 'in' ? 'Sisään...' : 'Ulos...'}</span>
			</div>
		{:else}
			<p class="instruction">
				{#if interaction.type === 'math' && mathProblem}
					{mathProblem.question}
				{:else}
					{interaction.instruction}
				{/if}
			</p>
			<input
				type={interaction.type === 'math' ? 'number' : 'text'}
				bind:value={input}
				on:keydown={(e) => e.key === 'Enter' && submit()}
				placeholder={interaction.type === 'clock' ? 'HH:MM' : ''}
				autofocus
			/>
			{#if error}
				<p class="error">{error}</p>
			{/if}
			<button class="submit" on:click={submit}>Valmis</button>
		{/if}
	</div>
</div>

<style>
	.modal-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.9);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
		padding: 2rem;
	}

	.modal {
		width: 100%;
		max-width: 360px;
		background: #1a1a1a;
		border-radius: var(--radius-lg);
		padding: 2rem;
		text-align: center;
		position: relative;
		overflow: hidden;
	}

	.timer-bar {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		height: 4px;
		background: rgba(255, 255, 255, 0.1);
	}
	.timer-fill {
		height: 100%;
		background: var(--intensity-warning);
		transition: width 1s linear;
	}

	h3 {
		font-size: 1.2rem;
		margin-bottom: 0.5rem;
	}

	.time-left {
		font-size: 2rem;
		font-weight: 200;
		color: var(--intensity-warning);
		margin-bottom: 1.5rem;
		font-variant-numeric: tabular-nums;
	}

	.instruction {
		font-size: 1.1rem;
		margin-bottom: 1.5rem;
		color: var(--text-dim);
	}

	input {
		width: 100%;
		padding: 1rem;
		font-size: 1.5rem;
		text-align: center;
		background: var(--input-bg);
		border: 1px solid var(--border-card);
		border-radius: var(--radius-sm);
		color: var(--text);
		margin-bottom: 1rem;
	}
	input:focus {
		border-color: rgba(30, 144, 255, 0.4);
	}

	.error {
		color: var(--danger);
		font-size: 0.9rem;
		margin-bottom: 0.75rem;
	}

	.submit {
		width: 100%;
		padding: 0.9rem;
		border-radius: var(--radius-sm);
		background: rgba(126, 178, 255, 0.2);
		border: 1px solid rgba(126, 178, 255, 0.3);
		color: var(--accent);
		font-size: 1rem;
		font-weight: 600;
	}

	.breathe-circle {
		width: 160px;
		height: 160px;
		border-radius: 50%;
		background: rgba(126, 178, 255, 0.15);
		border: 2px solid var(--accent);
		display: flex;
		align-items: center;
		justify-content: center;
		margin: 2rem auto;
		transform: scale(0.7);
		transition: transform 4s ease-in-out;
	}
	.breathe-circle.inhale {
		transform: scale(1);
	}
	.breathe-circle span {
		font-size: 1.2rem;
		color: var(--accent);
	}
</style>
