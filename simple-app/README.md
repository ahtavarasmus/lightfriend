# Lightfriend - Yksinkertainen asennus

## Mitä tarvitset

1. **Docker Desktop** - [Lataa tästä](https://www.docker.com/products/docker-desktop/)
2. **AI API-avain** - esim. [OpenRouter](https://openrouter.ai/keys) (ilmainen rekisteröityminen)

## Asennus ja käynnistys

```bash
# 1. Kloonaa tai kopioi tämä kansio
# 2. Avaa terminaali tässä kansiossa
# 3. Käynnistä:
./start.sh
```

Skripti kysyy sinulta:
- AI API-avaimen
- Admin-sähköpostin
- Admin-salasanan

Kaikki muut avaimet generoidaan automaattisesti.

## Käyttö

Avaa selaimessa: **http://localhost:3000**

## Komennot

| Komento | Kuvaus |
|---------|--------|
| `./start.sh` | Käynnistä sovellus |
| `./stop.sh` | Pysäytä sovellus |
| `docker compose logs -f core` | Katso logit |
| `docker compose restart core` | Käynnistä uudelleen |

## Nollaus

Jos haluat aloittaa alusta:
```bash
./stop.sh
docker compose down -v   # Poistaa kaiken datan
rm .env tuwunel.toml
./start.sh
```
