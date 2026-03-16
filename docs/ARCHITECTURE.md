# Lightfriend — Arkkitehtuuridokumentaatio

Tämä dokumentti selittää, miten Lightfriend toimii kokonaisuutena: miten iOS-appi, backend-palvelin ja tekoäly (Tinfoil.sh) keskustelevat keskenään.

---

## Yleiskuva

```
┌─────────────────────┐         ┌──────────────────────┐         ┌─────────────────────┐
│                     │  HTTP   │                      │  HTTP   │                     │
│    iOS-appi         │ ──────> │    Backend-palvelin   │ ──────> │   Tinfoil.sh AI     │
│    (WebView)        │ <────── │    (localhost:3000)   │ <────── │   (kimi-k2-5)       │
│                     │  JSON   │                      │  JSON   │                     │
└─────────────────────┘         └──────────────────────┘         └─────────────────────┘
        │                               │
        │                               ├── SQLite-tietokanta
        │                               ├── PostgreSQL (arkaluontoiset)
        │                               ├── Twilio (SMS/puhelut)
        │                               ├── Stripe (maksut)
        │                               └── ElevenLabs (puhesynteesi)
        │
        └── APNs push-notifikaatiot (Apple)
```

**Yksinkertaisesti:** iOS-appi on "näyttö", backend on "aivot", ja Tinfoil.sh on "tekoäly" joka vastaa kysymyksiin.

---

## 1. Backend-palvelin

### Mikä se on?

Backend on Rust-kielellä kirjoitettu palvelin, joka pyörii koneellasi ja kuuntelee HTTP-pyyntöjä portissa **3000**.

### Miten käynnistät?

```bash
cd backend && cargo run
```

Tämä käynnistää palvelimen osoitteeseen `http://localhost:3000`.

### Mitä se tekee?

- Vastaanottaa viestejä iOS-appilta
- Tarkistaa onko käyttäjä kirjautunut (JWT-token)
- Veloittaa krediittejä käyttäjän tililtä
- Lähettää viestin Tinfoil.sh AI:lle
- Palauttaa AI:n vastauksen takaisin appiin
- Hallinnoi käyttäjätilejä, maksuja ja integraatioita

### Tärkeimmät tiedostot

| Tiedosto | Kuvaus |
|---|---|
| `backend/src/main.rs` | Palvelimen käynnistys ja reitit (URL-osoitteet) |
| `backend/src/handlers/profile_handlers.rs` | Chat-viestien käsittely |
| `backend/src/handlers/auth_middleware.rs` | Kirjautumisen tarkistus |
| `backend/src/ai_config.rs` | Tinfoil.sh AI -yhteyden asetukset |
| `backend/src/repositories/` | Tietokantakyselyt |

---

## 2. iOS-appi

### Miten se toimii?

iOS-appi on käytännössä **WebView** — eli se näyttää web-sivun (HTML/JavaScript) natiivissa iOS-sovelluksessa. Tämä tarkoittaa:

1. Appi lataa `frontend.html`-tiedoston, joka sisältää koko käyttöliittymän
2. Kun käyttäjä tekee jotain (esim. lähettää viestin), JavaScript kutsuu backendiä
3. Kutsut kulkevat **Swift-sillan** (APIProxy) kautta, joka tekee oikean HTTP-pyynnön

```
┌──────────────────────────────────────────────────┐
│  iOS-appi                                        │
│                                                  │
│  ┌────────────────┐    ┌──────────────────────┐  │
│  │  WebView        │    │  APIProxy (Swift)    │  │
│  │  (frontend.html)│───>│  Välittää HTTP-      │──── HTTP ──> Backend
│  │                 │<───│  pyynnöt backendille  │<─── JSON ──
│  └────────────────┘    └──────────────────────┘  │
│                                                  │
└──────────────────────────────────────────────────┘
```

### Miksi Swift-silta?

Normaali web-sivu ei voi suoraan soittaa toiselle palvelimelle (CORS-rajoitus). Swift-silta ohittaa tämän rajoituksen — se tekee pyynnöt iOS:n natiivin verkkoyhteyden kautta.

### Palvelimen osoite

Appi yhdistää oletuksena osoitteeseen `http://localhost:3000`. Osoitteen voi vaihtaa apin asetuksissa (Settings-näkymä). Asetus tallentuu laitteen muistiin (`UserDefaults`).

### Tärkeimmät tiedostot

| Tiedosto | Kuvaus |
|---|---|
| `simple-app/ios/Lightfriend/ContentView.swift` | Pää-näkymä, APIProxy-silta, asetukset |
| `simple-app/ios/Lightfriend/LightfriendApp.swift` | Apin käynnistys, push-notifikaatiot |
| `simple-app/ios/Lightfriend/frontend.html` | Web-käyttöliittymä (Yew/WebAssembly) |

---

## 3. Kirjautuminen (Auth)

### Miten se toimii?

Lightfriend käyttää **JWT-tokeneita** (JSON Web Token) kirjautumiseen. Ajattele tokenia kuin sisäänpääsylippua — se todistaa kuka olet.

```
1. Käyttäjä kirjautuu sisään
   └─> Appi lähettää sähköposti + salasana backendille
   └─> POST /api/login

2. Backend tarkistaa tunnukset
   └─> Jos oikein: luo kaksi tokenia
       ├── access_token  (lyhytikäinen, ~15 min)
       └── refresh_token (pitkäikäinen, päivittää access_tokenin)

3. Appi tallentaa tokenit
   └─> localStorage selaimessa / WebView:ssä

4. Jokainen pyyntö sisältää tokenin
   └─> Authorization: Bearer eyJhbGciOi...
   └─> Backend tarkistaa tokenin aitouden ennen kuin tekee mitään
```

### Mitä tapahtuu ilman tokenia?

Backend hylkää pyynnön ja palauttaa virheen "401 Unauthorized". Appi ohjaa takaisin kirjautumissivulle.

---

## 4. Viestin kulku — askel askeleelta

Tämä on **tärkein osa**: mitä tapahtuu kun kirjoitat viestin appissa ja painat "lähetä".

```
Käyttäjä: "Mikä on sää Helsingissä?"
         │
         ▼
┌─ 1. iOS-appi ──────────────────────────────────────────────┐
│  JavaScript kutsuu: fetch("/api/chat/web", {               │
│    method: "POST",                                         │
│    headers: { Authorization: "Bearer {token}" },           │
│    body: { message: "Mikä on sää Helsingissä?" }           │
│  })                                                        │
│  → APIProxy (Swift) välittää pyynnön backendille           │
└────────────────────────────────────────────────────────────┘
         │
         ▼
┌─ 2. Backend vastaanottaa ──────────────────────────────────┐
│  a) Tarkistaa JWT-tokenin → kuka käyttäjä on?              │
│  b) Tarkistaa tilaustason → onko tilaus aktiivinen?        │
│  c) Veloittaa krediittejä:                                 │
│     • USA/Kanada: 0.5 krediittiä per viesti                │
│     • Eurooppa:   0.01 € per viesti                        │
│  d) Jos krediitit eivät riitä → virheilmoitus              │
└────────────────────────────────────────────────────────────┘
         │
         ▼
┌─ 3. Backend → Tinfoil.sh AI ───────────────────────────────┐
│  Backend rakentaa AI-pyynnön:                              │
│  • Järjestelmäviesti (system prompt): "Vastaa max 480      │
│    merkillä, nykyinen päivämäärä on..."                     │
│  • Käyttäjän konteksti: aikavyöhyke, sijainti              │
│  • Viimeisimmät viestit (keskusteluhistoria)               │
│  • Käytettävissä olevat työkalut (sää, YouTube, jne.)      │
│                                                            │
│  Lähettää: POST https://inference.tinfoil.sh/v1/chat/...   │
└────────────────────────────────────────────────────────────┘
         │
         ▼
┌─ 4. Tinfoil.sh AI vastaa ─────────────────────────────────┐
│  AI (kimi-k2-5 malli) päättää mitä tehdä:                  │
│                                                            │
│  Vaihtoehto A: Suora vastaus                               │
│  → "Helsinki: nyt 5°C, pilvistä, tuulta 3 m/s"            │
│                                                            │
│  Vaihtoehto B: Käyttää työkalua ensin                      │
│  → Kutsuu sää-työkalua → saa reaaliaikaisen datan          │
│  → Muotoilee vastauksen datan perusteella                   │
└────────────────────────────────────────────────────────────┘
         │
         ▼
┌─ 5. Vastaus palaa käyttäjälle ─────────────────────────────┐
│  Backend palauttaa JSON-vastauksen:                         │
│  {                                                         │
│    "message": "Helsinki: nyt 5°C, pilvistä...",            │
│    "credits_charged": 0.01,                                │
│    "media": null                                           │
│  }                                                         │
│                                                            │
│  → Swift vastaanottaa → välittää WebView:lle               │
│  → JavaScript näyttää vastauksen chatissa                   │
└────────────────────────────────────────────────────────────┘
```

---

## 5. Tinfoil.sh — tekoälypalvelu

### Mikä se on?

[Tinfoil.sh](https://tinfoil.sh) on pilvipalvelu, joka tarjoaa pääsyn tekoälymalleihin. Lightfriend käyttää sitä kaikkiin AI-vastauksiin.

### Miksi juuri Tinfoil?

Tinfoil tarjoaa yksityisyyttä suojaavia ominaisuuksia — viestejä ei tallenneta eikä käytetä mallien koulutukseen.

### Miten se on konfiguroitu?

- **Osoite:** `https://inference.tinfoil.sh/v1/chat/completions`
- **Malli:** `kimi-k2-5` (reasoning-malli, eli "ajattelee" ennen vastausta)
- **API-avain:** Asetetaan ympäristömuuttujana `TINFOIL_API_KEY`
- **Aikakatkaisu:** 120 sekuntia (pitkä, koska malli "ajattelee")
- **Uudelleenyritykset:** Enintään 3 kertaa jos yhteys katkeaa

### Vara-AI-palvelut (fallback)

Jos Tinfoil ei ole käytettävissä (esim. API-avain puuttuu), backend käyttää automaattisesti vaihtoehtoista palvelua:

```
Prioriteetti:
1. Tinfoil.sh    ← ensisijainen (yksityisin)
2. Anthropic     ← vaihtoehto (Claude-malli)
3. OpenRouter    ← viimeinen vaihtoehto (useat mallit)
```

Tämä tapahtuu automaattisesti — backend tarkistaa käynnistyessään mitkä API-avaimet on asetettu ja valitsee parhaan saatavilla olevan palvelun.

### Asetustiedosto

Tiedostossa `backend/src/ai_config.rs` määritellään:
- Mihin osoitteeseen pyynnöt lähetetään
- Mitä mallia käytetään
- Kuinka kauan odotetaan vastausta
- Miten virhetilanteet käsitellään
- Mikä AI-palvelu valitaan (prioriteettijärjestys)

---

## 6. Push-notifikaatiot (APNs)

Apple Push Notification service (APNs) mahdollistaa ilmoitusten lähettämisen iOS-laitteelle, vaikka appi ei olisi auki.

```
┌─────────┐    rekisteröinti     ┌─────────────┐
│ iOS-appi │ ──────────────────> │ Apple (APNs) │
│          │ <── device token ── │             │
└─────────┘                     └─────────────┘
     │                                │
     │ token                          │
     ▼                                │
┌──────────┐    push-viesti     ┌─────┘
│ Backend  │ ──────────────────>│
└──────────┘
```

### Miten se toimii askel askeleelta?

1. iOS-appi pyytää push-luvan käyttäjältä (ensimmäisellä käynnistyksellä)
2. Jos käyttäjä hyväksyy → Apple antaa **device tokenin** (uniikin tunnisteen laitteelle)
3. Appi lähettää tokenin backendille (`POST /api/push/register`)
4. Backend tallentaa tokenin tietokantaan (salattuna)
5. Kun backendilla on jotain ilmoitettavaa → se lähettää push-viestin APNs:n kautta

### Notifikaatiokategoriat

| Kategoria | Kuvaus |
|---|---|
| `message` | Chat-viestit |
| `reminder` | Muistutukset |
| `critical` | Tärkeät hälytykset |
| `digest` | Päivittäiset yhteenvedot |

### APNs-ympäristömuuttujat

| Muuttuja | Kuvaus |
|---|---|
| `APNS_KEY_PATH` | Polku Applen .p8-avaintiedostoon |
| `APNS_KEY_ID` | Avaimen tunniste (Apple Developer) |
| `APNS_TEAM_ID` | Tiimin tunniste (Apple Developer) |
| `APNS_TOPIC` | Apin bundle ID (esim. `ai.lightfriend.app`) |
| `APNS_SANDBOX` | `true` kehityksessä, `false` tuotannossa |

> **Huom:** Push-notifikaatiot toimivat ilman näitä asetuksia kehityksessä — backend ohittaa ne automaattisesti jos avaimet puuttuvat.

---

## 7. Ympäristömuuttujat

Ympäristömuuttujat ovat asetuksia, jotka backend lukee käynnistyessään. Ne määritellään tiedostossa `backend/.env`.

### Pakolliset

| Muuttuja | Kuvaus | Esimerkki |
|---|---|---|
| `DATABASE_URL` | SQLite-tietokannan polku | `database.db` |
| `JWT_SECRET_KEY` | Salainen avain kirjautumistokenien luomiseen | `pitkä-satunnainen-merkkijono` |
| `JWT_REFRESH_KEY` | Salainen avain refresh-tokenien luomiseen | `toinen-pitkä-merkkijono` |
| `ENCRYPTION_KEY` | Salausavain arkaluontoiselle datalle (base64) | `base64-koodattu-32-tavua` |
| `TINFOIL_API_KEY` | Tinfoil.sh API-avain tekoälyä varten | `tk_xxxxx` |
| `SERVER_URL` | Backendin julkinen URL | `https://esimerkki.com` |

### Valinnaiset (AI-varpalvelut)

| Muuttuja | Palvelu | Kuvaus |
|---|---|---|
| `ANTHROPIC_API_KEY` | Anthropic | Vara-AI (Claude), käytetään jos Tinfoil ei saatavilla |
| `OPENROUTER_API_KEY` | OpenRouter | Viimeinen vara-AI, useat mallit |

### Valinnaiset (integraatiot)

| Muuttuja | Palvelu | Kuvaus |
|---|---|---|
| `STRIPE_SECRET_KEY` | Stripe | Maksujen käsittely |
| `STRIPE_WEBHOOK_SECRET` | Stripe | Webhook-viestien todentaminen |
| `TWILIO_ACCOUNT_SID` | Twilio | SMS- ja puhelupalvelu |
| `TWILIO_AUTH_TOKEN` | Twilio | Twilio-todentaminen |
| `ELEVENLABS_API_KEY` | ElevenLabs | Puhesynteesi |

### Miten asetat?

1. Kopioi esimerkkitiedosto: `cp backend/.env.example backend/.env`
2. Avaa `backend/.env` tekstieditorissa
3. Täytä arvot (saat ne palveluntarjoajilta)
4. Käynnistä backend uudelleen: `cargo run`

> **Tärkeää:** `.env`-tiedostoa ei saa koskaan laittaa Gitiin! Se sisältää salaisia avaimia.

---

## 8. Muut viestikanavat

iOS-apin lisäksi käyttäjät voivat keskustella Lightfriendin kanssa myös:

### SMS (Twilio)

```
Käyttäjän puhelin ──SMS──> Twilio ──webhook──> Backend ──> Tinfoil AI
                  <──SMS── Twilio <──vastaus─── Backend <── Tinfoil AI
```

Käsittely: `backend/src/api/twilio_sms.rs`

### Web-selain

Frontend toimii myös suoraan selaimessa osoitteessa `http://localhost:8080` (kehityksessä).

---

## Yhteenveto

| Komponentti | Rooli | Teknologia |
|---|---|---|
| iOS-appi | Käyttöliittymä | Swift + WebView |
| Frontend | Web-käyttöliittymä | Rust (Yew + WebAssembly) |
| Backend | Palvelin ja logiikka | Rust (Axum) |
| Tietokanta | Datan tallennus | SQLite + PostgreSQL |
| Tekoäly | Vastausten generointi | Tinfoil.sh (kimi-k2-5) |
| SMS | Viestikanava | Twilio |
| Maksut | Tilaukset ja krediitit | Stripe |
| Push | Ilmoitukset | Apple APNs |
