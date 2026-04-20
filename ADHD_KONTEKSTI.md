Tässä konteksti edellisestä sessiosta. Lisäsin repoon nämä uudet tiedostot:

- src/lib/core/events.ts - event bus (intensity-changed, interaction-required jne.)
- src/lib/core/storage.ts - localStorage tallennus intensiteettiasetuksille
- src/lib/modules/iltavahti/time-engine.ts - deadline-laskenta, intensiteettitasot
- src/lib/modules/iltavahti/interaction-picker.ts - anti-habituaatio, 4 interaktiotyyppiä
- src/lib/modules/iltavahti/intensity-handler.ts - yhdistää notifikaatiot/värinä/wakelock
- src/lib/modules/iltavahti/InteractionModal.svelte - pakotettu tehtävä countdownilla
- src/lib/modules/iltavahti/IntensityOverlay.svelte - fullscreen overlay
- src/routes/tervetuloa/+page.svelte - 5-vaiheinen onboarding

## Miten tiedostot toimivat

### events.ts
Tyypitetty pub/sub. Tapahtumat: intensity-changed, interaction-required, interaction-completed, interaction-timeout, bedtime-reached, settings-updated.

### storage.ts
localStorage avaimella "adhd-iltavahti-v2". Tallentaa IntensitySettings (wakeUpTime, sleepHours, intensityPreference light/medium/hard, selectedSymptoms, onboardingDone) ja InteractionRecord (type, timestamp, durationMs, completed).

### time-engine.ts
- calculateDeadline(wakeTime, sleepHours) - laskee nukkumaanmenoajan seuraavasta herätyksestä taaksepäin
- sleepIfNow(wakeTime) - tunnit unta jos nukahtaa NYT
- getIntensityLevel(hoursRemaining) - >2h calm, >1h gentle, >0.5h warning, >0h urgent, muuten overdue
- getIntensityColor(level) - calm:#7eb2ff, gentle:#ffab40, warning:#ff7043, urgent:#ff5252, overdue:#e040fb

### interaction-picker.ts
4 tyyppiä: clock (kirjoita kellonaika, 15s), math (laskutehtävä, 20s), acknowledge (kirjoita "Menen nukkumaan", 10s), breathe (hengitysharjoitus, 15s). Anti-habituaatio: painotettu satunnainen - seuraa 5 viimeistä interaktiota ja painottaa harvemmin käytettyjä.

### intensity-handler.ts
Käyttää olemassa olevia haptic/wakeLock-funktioita src/lib/iltavahti.ts:stä. Tasot:
- calm: ei toimintaa
- gentle: browser-notifikaatio (paitsi kevyt-tasolla)
- warning: notifikaatio + värinä + interaktio 5min välein (keskikova+)
- urgent: wake lock + interaktio 2min välein
- overdue: interaktio 1min välein

### InteractionModal.svelte
Fullscreen modal countdown-timerillä. Pakottaa suorittamaan tehtävän aikarajan sisällä. Jos aika loppuu -> timeout -> intensiteetti nousee. Svelte legacy mode (runes={false}).

### IntensityOverlay.svelte
Läpinäkyvä fullscreen overlay urgent/overdue-tasoilla. Pulsoiva rengas. "MENE NUKKUMAAN" / "OLET MYÖHÄSSÄ".

### tervetuloa/+page.svelte
5-vaiheinen onboarding: 1) Intro "Ilta venyy", 2) ADHD-oireiden tunnistus, 3) Herätysaika + unitunnit, 4) Intensiteettitaso, 5) Esikatselu. Tallentaa localStorage:en ja ohjaa /iltavahti.

## Seuraavat askeleet

Integroi nämä olemassa olevaan iltavahti-sivuun:
1. Lisää intensiteettijärjestelmä (import time-engine + intensity-handler)
2. Näytä "tuntia unta jäljellä" -laskuri
3. Aktivoi InteractionModal kun intensity-handler emittoi interaction-required
4. Näytä IntensityOverlay urgent/overdue-tasoilla
5. Ensimmäisellä käynnistyskerralla ohjaa /tervetuloa onboardingiin
6. Lisää asetuksiin intensiteettitason muutos

## Käyttäjän palaute

Interaktioiden pitää olla NOPEITA (5-15s) mutta PAKOLLISIA. Ei saa tuntua työläältä mutta ei voi ohittaa. Countdown timer pakottaa toimimaan nopeasti.
