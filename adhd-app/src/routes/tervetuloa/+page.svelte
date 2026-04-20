<script lang="ts">
	import { goto } from '$app/navigation';
	import { settings, type IntensityPreference } from '$lib/core/state';
	import { saveSettings } from '$lib/core/storage';

	let step = 1;
	let fadeIn = true;

	// Step 2
	const symptoms = [
		'Puhelimen selaus venyttää iltaa',
		'En huomaa ajan kulumista',
		'Aamulla ei jaksa nousta',
		'Nukahtaminen kestää kauan',
		'Hyperfokus illalla',
		'Ruutuaika räjähtää'
	];
	let selectedSymptoms: string[] = [];

	// Step 3
	let wakeUpTime = '07:00';
	let sleepHours = 8;

	// Step 4
	let intensityPreference: IntensityPreference = 'medium';

	function toggleSymptom(s: string) {
		if (selectedSymptoms.includes(s)) {
			selectedSymptoms = selectedSymptoms.filter((x) => x !== s);
		} else {
			selectedSymptoms = [...selectedSymptoms, s];
		}
	}

	function nextStep() {
		fadeIn = false;
		setTimeout(() => {
			step++;
			fadeIn = true;
		}, 200);
	}

	$: deadline = (() => {
		const [h, m] = wakeUpTime.split(':').map(Number);
		const totalMinutes = h * 60 + m - sleepHours * 60;
		const dh = Math.floor(((totalMinutes % (24 * 60)) + 24 * 60) % (24 * 60) / 60);
		const dm = ((totalMinutes % 60) + 60) % 60;
		return `${dh.toString().padStart(2, '0')}:${dm.toString().padStart(2, '0')}`;
	})();

	async function finish() {
		const newSettings = {
			onboardingDone: true,
			wakeUpTime,
			sleepHours,
			intensityPreference,
			selectedSymptoms
		};
		settings.set(newSettings);
		await saveSettings(newSettings);
		await goto('/iltavahti');
	}
</script>

<div class="onboarding" class:fade-in={fadeIn}>
	{#if step === 1}
		<div class="step step-1">
			<h1 class="gradient-text">Ilta venyy.</h1>
			<h1 class="gradient-text">Aamu tulee silti.</h1>
			<p class="subtitle">Iltavahti auttaa sinua menemään nukkumaan ajoissa.</p>
			<p class="detail">Ei to-do-listoja. Ei syyllistämistä. Vain rehellinen kello ja pientä painostusta.</p>
			<button class="cta" on:click={nextStep}>Aloitetaan</button>
		</div>

	{:else if step === 2}
		<div class="step step-2">
			<h2>Tunnistatko näitä?</h2>
			<p class="subtitle">Valitse kaikki jotka sopivat sinuun</p>
			<div class="symptom-grid">
				{#each symptoms as symptom}
					<button
						class="symptom-card"
						class:selected={selectedSymptoms.includes(symptom)}
						on:click={() => toggleSymptom(symptom)}
					>
						{symptom}
					</button>
				{/each}
			</div>
			<button class="cta" on:click={nextStep}>
				{selectedSymptoms.length > 0 ? 'Jatka' : 'Ohita'}
			</button>
		</div>

	{:else if step === 3}
		<div class="step step-3">
			<h2>Milloin heräät?</h2>
			<div class="time-input">
				<input type="time" bind:value={wakeUpTime} />
			</div>
			<h2>Montako tuntia unta tarvitset?</h2>
			<div class="sleep-hours">
				<button on:click={() => sleepHours = Math.max(5, sleepHours - 0.5)}>−</button>
				<span class="hours-display">{sleepHours}h</span>
				<button on:click={() => sleepHours = Math.min(10, sleepHours + 0.5)}>+</button>
			</div>
			<div class="deadline-preview">
				<p>Nukkumaanmenoaika:</p>
				<p class="deadline-time">{deadline}</p>
			</div>
			<button class="cta" on:click={nextStep}>Jatka</button>
		</div>

	{:else if step === 4}
		<div class="step step-4">
			<h2>Kuinka kovaa painostusta haluat?</h2>
			<div class="intensity-options">
				<button
					class="intensity-card"
					class:selected={intensityPreference === 'light'}
					on:click={() => intensityPreference = 'light'}
				>
					<span class="intensity-emoji">🌙</span>
					<span class="intensity-label">Kevyt</span>
					<span class="intensity-desc">Lempeät muistutukset</span>
				</button>
				<button
					class="intensity-card"
					class:selected={intensityPreference === 'medium'}
					on:click={() => intensityPreference = 'medium'}
				>
					<span class="intensity-emoji">⚡</span>
					<span class="intensity-label">Keskikova</span>
					<span class="intensity-desc">Muistutukset + pakotetut tehtävät</span>
				</button>
				<button
					class="intensity-card"
					class:selected={intensityPreference === 'hard'}
					on:click={() => intensityPreference = 'hard'}
				>
					<span class="intensity-emoji">🔥</span>
					<span class="intensity-label">Kova</span>
					<span class="intensity-desc">Kaikki keinot käyttöön</span>
				</button>
			</div>
			<button class="cta" on:click={nextStep}>Jatka</button>
		</div>

	{:else if step === 5}
		<div class="step step-5">
			<h2>Valmista!</h2>
			<div class="preview-number">
				<span class="big-number">{sleepHours.toFixed(1)}</span>
				<span class="big-label">tuntia unta jos menet nyt</span>
			</div>
			<p class="subtitle">Tämä numero päivittyy reaaliajassa.</p>
			<p class="detail">Kun aika loppuu, Iltavahti alkaa painostamaan.</p>
			<button class="cta" on:click={finish}>Käynnistä Iltavahti</button>
		</div>
	{/if}
</div>

<style>
	.onboarding {
		min-height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 2rem;
		opacity: 0;
		transition: opacity 0.3s ease;
	}
	.onboarding.fade-in { opacity: 1; }

	.step {
		max-width: 400px;
		width: 100%;
		text-align: center;
	}

	.gradient-text {
		font-size: 2rem;
		font-weight: 700;
		background: linear-gradient(45deg, #fff, #7eb2ff);
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
		line-height: 1.3;
	}

	h2 {
		font-size: 1.4rem;
		font-weight: 600;
		margin-bottom: 1rem;
	}

	.subtitle {
		color: var(--text-dim);
		margin: 1rem 0;
		font-size: 1rem;
	}

	.detail {
		color: var(--text-dim);
		font-size: 0.9rem;
		margin-bottom: 2rem;
	}

	.cta {
		width: 100%;
		padding: 1rem;
		border-radius: var(--radius-sm);
		background: linear-gradient(135deg, rgba(126, 178, 255, 0.2), rgba(30, 144, 255, 0.3));
		border: 1px solid rgba(126, 178, 255, 0.3);
		color: var(--accent);
		font-size: 1.1rem;
		font-weight: 600;
		margin-top: 1.5rem;
		transition: all 0.2s;
	}
	.cta:active {
		transform: scale(0.98);
		background: linear-gradient(135deg, rgba(126, 178, 255, 0.3), rgba(30, 144, 255, 0.4));
	}

	/* Step 2: Symptoms */
	.symptom-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.75rem;
		margin: 1.5rem 0;
	}
	.symptom-card {
		padding: 1rem 0.75rem;
		border-radius: var(--radius-md);
		background: var(--bg-card);
		border: 1px solid var(--border-card);
		color: var(--text-dim);
		font-size: 0.85rem;
		transition: all 0.2s;
	}
	.symptom-card.selected {
		border-color: var(--accent);
		color: var(--text);
		background: rgba(126, 178, 255, 0.1);
	}

	/* Step 3: Time */
	.time-input { margin: 1.5rem 0 2rem; }
	.time-input input[type="time"] {
		font-size: 2rem;
		background: var(--input-bg);
		border: 1px solid var(--border-card);
		border-radius: var(--radius-sm);
		color: var(--text);
		padding: 0.75rem 1.5rem;
		text-align: center;
	}
	.time-input input[type="time"]:focus {
		border-color: rgba(30, 144, 255, 0.4);
	}

	.sleep-hours {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: 1.5rem;
		margin: 1.5rem 0;
	}
	.sleep-hours button {
		width: 48px;
		height: 48px;
		border-radius: 50%;
		background: var(--bg-card);
		border: 1px solid var(--border-card);
		color: var(--text);
		font-size: 1.5rem;
	}
	.hours-display {
		font-size: 2.5rem;
		font-weight: 300;
		font-variant-numeric: tabular-nums;
		min-width: 80px;
	}

	.deadline-preview {
		margin: 1.5rem 0;
		padding: 1rem;
		background: var(--bg-card);
		border-radius: var(--radius-md);
		border: 1px solid var(--border-card);
	}
	.deadline-preview p { color: var(--text-dim); font-size: 0.9rem; }
	.deadline-time {
		font-size: 1.8rem;
		font-weight: 600;
		color: var(--accent) !important;
		margin-top: 0.25rem;
	}

	/* Step 4: Intensity */
	.intensity-options {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		margin: 1.5rem 0;
	}
	.intensity-card {
		display: flex;
		align-items: center;
		gap: 1rem;
		padding: 1rem 1.25rem;
		border-radius: var(--radius-md);
		background: var(--bg-card);
		border: 1px solid var(--border-card);
		color: var(--text);
		text-align: left;
		transition: all 0.2s;
	}
	.intensity-card.selected {
		border-color: var(--accent);
		background: rgba(126, 178, 255, 0.1);
	}
	.intensity-emoji { font-size: 1.5rem; }
	.intensity-label { font-weight: 600; flex: 1; }
	.intensity-desc { font-size: 0.8rem; color: var(--text-dim); }

	/* Step 5: Preview */
	.preview-number { margin: 2rem 0; }
	.big-number {
		font-size: 5rem;
		font-weight: 200;
		font-variant-numeric: tabular-nums;
		display: block;
		background: linear-gradient(45deg, #fff, #7eb2ff);
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
	}
	.big-label {
		font-size: 1rem;
		color: var(--text-dim);
	}
</style>
