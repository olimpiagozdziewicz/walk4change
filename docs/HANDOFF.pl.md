# SeaSteps (walk4change) — dokumentacja przekazania

Kompletny opis działania aplikacji: frontend, backend, baza danych, wdrożenia i
lokalne uruchomienie. Sekrety (hasła, klucze) **nie** są tu zawarte — żyją w
konfiguracji Azure App Service (backend), env projektu Vercel (frontend) i w
prywatnym sejfie właścicielki. Nigdy w repo ani w czacie.

> **Zaktualizowano 2026-07-09.** Infrastruktura z czasów hackathonu (homelab
> Kamila, Cloudflare Tunnel, k3s/ArgoCD, stary projekt Supabase
> `vjsjxdqnmhyrglsfqvvp`) już NIE istnieje operacyjnie — przejęcie 06–07.07.2026.
> Wszystko działa na kontach Olimpii: Azure (backend), Vercel (frontend),
> Supabase `plncauubrwbfbavcejgs`.

---

## 0. Stan bezpieczeństwa / multi-user (2026-07-10)

Pełny audyt żyje w workspace (repo `agenci`): `seasteps/spec/2026-07-10-audyt-multiuser-i-prebuild-social.md`, sekcja STATUS NAPRAW.

- **🔴 RLS na warstwie społecznej — NAPRAWIONE na prodzie.** Migracje `0005_social`/`0006_feed_social` utworzyły `messages`/`eco_likes`/`eco_comments` BEZ RLS → publiczny klucz anon czytał je przez PostgREST (potwierdzone live 10.07: anon-GET = 200, prywatne wiadomości 1:1 wyciekały). Włączono RLS + REVOKE anon/authenticated (Supabase Management API); po fixie anon = 401. Fix zastosowany **out-of-band**; trwały in-band = migracja `0007_rls_social.sql` na gałęzi `security-rls-2026-07-10` (NIE zmergowana). Backend łączy się jako `postgres` → omija RLS. **Zasada: każda nowa tabela MUSI mieć `ENABLE ROW LEVEL SECURITY` w migracji.**
- **🔴 Do zrobienia PRZED wpuszczeniem realnych obcych userów (kod Rust, kontrolowany merge, nie blind):** rate-limiter musi czytać `X-Forwarded-For` (bierze IP z socketa → za proxy Azure ~5 userów czatu = 429 dla wszystkich); brak unfriend/block (raz przyjęty „znajomy" = dożywotni kanał nękania); bug rejoin-po-leave (`left_at` nie czyszczony). Lista 🟠/🟡 w audycie.
- **Sufit 1 instancji B1: ~100–300 równoczesnych spacerowiczów** (pęka pula DB=10; sesje grupowe ~N² → ~10–15 pełnych grup). Sufit architektoniczny: hub WS w pamięci = nie skalować w poziomie bez Redis pub/sub / sticky sessions.
- **Build lokalny:** Rust JEST na maszynie „jazda" (rustc/cargo 1.97 + clippy; przez `. ~/.cargo/env`; repo = kopia Syncthing `/home/olimpia/asystenci/BIZNES/projekty/seasteps/app/backend`, nie ruszać tam gita) → `cargo check`/clippy PRZED merge, koniec ślepych pushów.

---

## 1. Przegląd architektury

```
  Przeglądarka (PWA)
        │  HTTPS
        ▼
  seasteps.pl (+ www → apex) ────►  Vercel (statyczny frontend)
        │                             projekt `seasteps-app`, konto olimpia-intuflow
        │                             • landing  /
        │                             • aplikacja /app/  (React + Vite)
        │
        │  REST + WebSocket (HTTPS/WSS)
        ▼
  Azure App Service (Linux B1, kontener)
  if-app-walk4change-prod-pl  •  RG if-rg-walk4change-prod-pl
  1 instancja NA SZTYWNO (hub WS w pamięci — NIE skalować)
        │
        ▼
  Supabase `plncauubrwbfbavcejgs` (org SeaSteps, konto admin@seasteps.pl)
  • PostgreSQL + PostGIS (session pooler)
  • Auth (magic-link OTP)
  • Storage (bucket eco-photos — zdjęcia eko)
```

Trzy niezależne elementy:

1. **Frontend** — React/Vite PWA, Vercel projekt `seasteps-app` → `seasteps.pl`.
2. **Backend** — Rust/Axum REST + WebSocket, kontener na **Azure App Service**
   (`https://if-app-walk4change-prod-pl.azurewebsites.net`, health `/api/v1/health`).
3. **Supabase** — PostgreSQL (z PostGIS), Auth (magic-link) i Storage (zdjęcia).
   RLS włączone deny-all na tabelach public (audyt 08.07.2026) — backend łączy się
   jako `postgres` i je omija; Data API nie wystawia tych tabel.

Frontend i backend są w jednym repo: **`olimpiagozdziewicz/walk4change`**
(transfer z `h4cstolik3` 07.07.2026; konto przemianowane z `olimpia-intuflow`
09.07.2026 — stare URL-e redirectują). Manifesty k8s/Helm Kamila — nieużywane.

---

## 2. Backend — Rust / Axum

Katalog: `backend/`. Workspace Cargo, główny crate: `crates/api`
(`walk4change-api`).

### 2.1 Struktura

```
backend/
├── crates/api/src/
│   ├── lib.rs            # budowa routera + middleware (CORS, rate-limit, security headers)
│   ├── main.rs           # bootstrap: config, pula DB, migracje, serwer
│   ├── auth/             # JWT, ekstraktor AuthUser, handlery logowania/rejestracji/magic-link
│   ├── routes/           # handlery REST: profile, stats, walks, friends, leaderboard, rewards, eco
│   ├── scoring/          # silnik punktacji (config, engine, repo)
│   ├── ws/               # WebSocket: handler, hub (broadcast), protokół (ramki)
│   ├── repo/             # dostęp do DB
│   ├── db.rs             # pula + sqlx::migrate!()
│   ├── mail.rs           # wysyłka magic-link (SMTP)
│   └── ...
├── migrations/           # migracje SQL (auto-aplikowane przy starcie)
├── Dockerfile
├── Makefile              # cele lokalne: demo, up, down, seed, logs
└── deploy/homelab.sh     # LEGACY (stary homelab) — prod = Azure, patrz §6
```

### 2.2 Główne endpointy REST (`/api/v1`)

| Metoda + ścieżka | Opis | Auth |
|---|---|---|
| `GET /health` | health check | nie |
| `POST /auth/register` | rejestracja (email + hasło) | nie |
| `POST /auth/login` | logowanie | nie |
| `POST /auth/logout` | wylogowanie | tak |
| `POST /auth/magic/request` | wyślij magic-link na email | nie |
| `POST /auth/magic/verify` | zweryfikuj token z linku | nie |
| `POST /auth/supabase` | wymiana sesji Supabase na JWT aplikacji | nie |
| `GET /me` / `PATCH /me` | profil zalogowanego | tak |
| `GET /me/stats` | statystyki: dziś + łącznie + streak | tak |
| `POST /walks` | rozpocznij spacer (zwraca `join_code`) | tak |
| `POST /walks/join-by-code` | dołącz do spaceru po kodzie | tak |
| `GET /walks/:id` | szczegóły spaceru | tak |
| `POST /walks/:id/{join,leave,stop}` | dołącz / opuść / zakończ | tak |
| `GET /walks/:id/track` | (WS) strumień pozycji | tak |
| `GET /leaderboard` | ranking globalny | tak |
| `GET /rewards`, `POST /rewards/:id/redeem`, `GET /me/redemptions` | nagrody | tak |
| `GET /eco/reports`, `POST /eco/reports` | zgłoszenia eko (lista + utworzenie) | tak |
| `GET /me/eco-reports` | moje zgłoszenia eko | tak |
| `GET /ws` | WebSocket (live feed spaceru) | tak (przez ramkę Auth) |

### 2.3 Autentykacja

Dwie ścieżki, obie kończą się **JWT aplikacji** (Bearer) zapisanym w `localStorage`:

1. **Email + hasło** — `POST /auth/login` / `/auth/register`.
2. **Magic-link (Supabase OTP)** — frontend prosi Supabase o link, użytkownik
   klika, wraca na `/app/auth/magic`, frontend wymienia sesję Supabase na JWT
   aplikacji przez `POST /auth/supabase`.

JWT podpisany sekretem `JWT_SECRET`. Ekstraktor `AuthUser` weryfikuje token na
chronionych endpointach (brak/niepoprawny → 401).

### 2.4 Silnik punktacji (scoring)

Punkty naliczane z **realnego dystansu GPS** (nie z kroków na urządzeniu). Każdy
ping GPS → segment między poprzednim a obecnym punktem → punktacja.

`scoring/engine.rs` — `score_segment`:

- **Mnożniki:** spacer z kimś ×1.5 (para) / ×2.0 (grupa 3+), strefa natury ×3.
  Domyślnie mnożą się (stack).
- `meters_per_point` = 100 m / punkt.
- **Zabezpieczenia przeciw oszustwom / szumowi GPS:**
  - `max_speed_mps` (domyślnie 8 m/s) — segment szybszy = teleport → 0.
  - `min_segment_meters` (domyślnie 5 m) — **deadband na drgania GPS**: segment
    krótszy traktowany jako stanie w miejscu → 0.
  - `max_accuracy_meters` (domyślnie 35 m) — pingi o słabym fixie GPS są
    odrzucane (nie tworzą segmentu).
  - `max_points_per_second` — pułap punktów/s.

Wszystkie progi konfigurowalne zmiennymi `SCORING_*` (bez przebudowy obrazu).

Strefy natury to poligony PostGIS (`nature_zones`); mnożnik z `ST_Covers`.

### 2.5 WebSocket (live spacer)

`ws/handler.rs` + `ws/hub.rs`. Klient łączy się z `/api/v1/ws`, wysyła ramkę
`Auth` (JWT), potem `Subscribe { session_id }` i `Ping { lat, lng, accuracy, ... }`.
Serwer punktuje ping i rozsyła `PingScored` do wszystkich w sesji.

> ⚠️ **Hub jest w pamięci procesu** → backend musi działać jako **pojedyncza
> instancja**. Wiele replik nie współdzieliłoby broadcastu.

### 2.6 Baza i migracje

- PostgreSQL = **Supabase** (session pooler), z rozszerzeniem **PostGIS**.
- Migracje w `backend/migrations/*.sql` aplikują się **automatycznie przy starcie**
  (`sqlx::migrate!()`). Kolejne pliki: `0001_init`, `0002_hardening`,
  `0003_magic_links`, `0004_eco_reports`.
- Tabele m.in.: `users`, `friendships`, `nature_zones`, `walk_sessions`,
  `walk_participants`, `location_pings`, `user_totals`, `rewards`,
  `magic_links`, `eco_reports`.

### 2.7 Zmienne środowiskowe backendu (nazwy, bez wartości)

W Azure App Service → Configuration (wartości w sejfie właścicielki):

| Zmienna | Opis |
|---|---|
| `DATABASE_URL` | connection string Supabase (session pooler) |
| `JWT_SECRET` | sekret do podpisu JWT (min. 32 znaki) |
| `BIND_ADDR` | adres nasłuchu (np. `0.0.0.0:8080`) |
| `CORS_ALLOWED_ORIGINS` | dozwolone originy (frontend) |
| `APP_URL` | bazowy URL aplikacji (do linków w mailach) |
| `SMTP_HOST/PORT/USER/PASS/FROM` | wysyłka magic-link |
| `SUPABASE_URL`, `SUPABASE_ANON_KEY` | wymiana sesji Supabase (anon key jest publiczny) |
| `SCORING_*` | (opcjonalne) progi punktacji |

---

## 3. Frontend — React / Vite (PWA)

Katalog: `web/`. Aplikacja serwowana pod `/app/` (Vite `base: '/app/'`);
landing (`index.html`) pod `/`.

### 3.1 Struktura

```
web/src/
├── App.tsx               # routing (react-router); RequireAuth chroni aplikację
├── main.tsx              # bootstrap + rejestracja service workera (PWA)
├── screens/              # Home, Walk, Community, Events, Profile, Eco, History, Login, MagicVerify, Partners
├── components/           # AppShell, BottomNav, Sidebar, LiveMap, InstallModal, ui, ...
├── hooks/useStepCounter  # liczenie kroków z dystansu GPS
└── lib/
    ├── http.ts           # klient REST (koperty data/error, JWT)
    ├── api.ts            # warstwa danych + adaptery; fallback na mocki gdy brak backendu
    ├── auth.ts           # login/register/logout, magic-link, wymiana sesji Supabase
    ├── ws.ts             # klient WebSocket
    └── supabase.ts       # klient Supabase (tylko magic-link + Storage)
```

### 3.2 Tryb mock vs. backend

`lib/http.ts`: jeśli `VITE_API_BASE` puste → tryb **mock** (dane z `api.ts`).
Ustawione → realne wywołania REST z JWT. Pozwala rozwijać UI bez backendu.

### 3.3 Ekrany (najważniejsze)

- **Home (Start)** — statystyki dziś (kroki/punkty/streak), pierścień postępu,
  szybkie akcje, partnerzy. Odświeża statystyki przy `focus`/`visibilitychange`.
- **Walk (Spacer)** — start/dołącz po kodzie, live GPS, mapka śladu (`LiveMap`),
  licznik kroków/metrów/punktów, podsumowanie po zakończeniu.
- **Eco (Eko)** — zgłoś problem / pochwal się sprzątaniem; zdjęcia ładowane
  bezpośrednio do Supabase Storage, lista z miniaturami.
- **Profile (Profil)** — edycja nazwy/zainteresowań, odznaki, moje zgłoszenia eko.
- **Login / MagicVerify** — logowanie hasłem lub magic-linkiem.

### 3.4 PWA / instalacja

- Manifest `web/public/manifest.webmanifest`, service worker `sw.js`
  (instalowalność + offline), zakres `/app/`.
- `InstallModal` pokazuje okienko instalacji: natywny przycisk (Chrome/Android),
  instrukcja Share-sheet (iOS Safari) lub menu (desktop).
- ⚠️ **iOS:** instalacja PWA działa **tylko w Safari** — Chrome/Google app na
  iPhone nie mają „Do ekranu początkowego".

### 3.5 Zmienne środowiskowe frontendu (build-time)

| Zmienna | Opis |
|---|---|
| `VITE_API_BASE` | publiczny URL backendu (bez końcowego `/`) |
| `VITE_SUPABASE_URL` | URL projektu Supabase |
| `VITE_SUPABASE_ANON_KEY` | publiczny klucz anon Supabase |

Wstrzykiwane przy buildzie (Vite). Brak `VITE_API_BASE` = tryb mock.

---

## 4. Supabase

- **Projekt: `plncauubrwbfbavcejgs`** (org SeaSteps, konto admin@seasteps.pl,
  eu-west-1, Free) — zmigrowany 07.07.2026 ze starego `vjsjxdqnmhyrglsfqvvp`
  (konto hackathonowe bez dostępu; stary projekt = zamrożony backup, nie ruszać).
- **PostgreSQL** — główna baza backendu (przez **session pooler**, port 5432).
- **Auth** — magic-link / OTP (frontend używa `supabase-js` tylko do tego;
  potem wymiana na JWT aplikacji). site_url `https://seasteps.pl`; redirecty:
  `seasteps.pl/app/auth/magic` + `seasteps-app.vercel.app/app/auth/magic`.
  Wbudowany mailer ma limit ~3–4/h — docelowo SMTP Brevo (domena seasteps.pl
  autoryzowana, nadawca `noreply@seasteps.pl` zweryfikowany 08.07; zostało:
  env SMTP na Azure + feature `email_verified`).
- **Storage** — bucket **`eco-photos`** (publiczny; limit 5 MB + whitelist MIME
  od audytu 08.07). Zdjęcia eko ładowane są **bezpośrednio z przeglądarki**
  do Storage (API ma limit body 64 KiB, więc zdjęcia nie przechodzą przez
  backend — w bazie trzymamy tylko URL-e).
- **RLS** — włączone deny-all na tabelach public (audyt 08.07); backend jako
  `postgres` bypassuje. PostgREST/Data API nie służy do tych tabel.

---

## 5. Lokalne uruchomienie

### Backend (Docker + lokalny Postgres)

```bash
cd backend
make up      # Postgres (PostGIS) + API na :8080
make seed    # dane demo: użytkownicy ana@/bek@ (hasło demodemo), strefy, nagrody
make demo    # pełne demo: stack + spacer + link + dwóch spacerowiczów
make logs    # logi API
make down    # zatrzymaj (ARGS=--purge by wyczyścić wolumen DB)
```

Wymaga lokalnego `.env` z `DATABASE_URL` i `JWT_SECRET` (patrz `.env.example`).

### Frontend

```bash
cd web
npm install
npm run dev          # tryb deweloperski (domyślnie tryb mock, bez backendu)
# z backendem:
VITE_API_BASE=http://localhost:8080 npm run dev
```

---

## 6. Wdrożenia (deploy)

### Frontend → Vercel (`seasteps.pl`) — ręczny deploy

Projekt Vercel: **`seasteps-app`** (konto olimpia-intuflow). **Brak git
integration** — push NIE triggeruje builda; deploy odpalasz ręcznie:

```bash
# z katalogu głównego repo (CLI zlinkowane z projektem seasteps-app):
npx vercel deploy --prod
```

Root `vercel.json` każe Vercelowi budować przez `scripts/build-site.sh`
(landing `/` + apka `/app/` + `privacy.html` + rewrites SPA).
Env build-time w projekcie Vercel (Production) — **komplet 3 przed każdym
buildem** (lekcja z incydentu 30.06): `VITE_API_BASE` (URL Azure),
`VITE_SUPABASE_URL`, `VITE_SUPABASE_ANON_KEY`.

### Backend → Azure App Service — automatyczne CI/CD

```
push (backend/**) → main
        │
        ▼
GitHub Actions: build obrazu Docker (backend/Dockerfile)
        → push do ghcr.io/olimpia-intuflow/walk4change-api (public)
        │
        ▼
webhook CD (sekret repo: AZURE_CD_WEBHOOK)
        → App Service if-app-walk4change-prod-pl ściąga :latest i restartuje
```

- Zweryfikowane e2e 07.07.2026 (build → ghcr → webhook → health + login 200).
- ⚠️ **NIEZWERYFIKOWANE po rename konta GitHub (09.07):** namespace ghcr mógł
  zmienić się na `olimpiagozdziewicz/` — przy następnym deployu backendu
  sprawdzić run CI i ew. przepiąć ścieżkę obrazu w App Service.
- App Service: Web Sockets ON, Always On ON, healthCheckPath `/api/v1/health`,
  **1 instancja na sztywno** (hub WS w pamięci).
- **Migracje** bazy aplikują się przy starcie kontenera (`sqlx::migrate!()`).
- Sekrety/env backendu: App Service → Configuration (patrz §2.7).

> Lokalny/awaryjny wariant: `backend/deploy/homelab.sh` uruchamia backend jako
> pojedynczy kontener Docker na `:8080` (legacy — stary homelab; do dev/demo).

---

## 7. Znane ograniczenia / pułapki

- **Backend = pojedyncza instancja** (hub WS w pamięci). Skalowanie wymaga
  współdzielonego broadcastu (np. Redis pub/sub).
- **iOS PWA** instaluje się tylko w **Safari** (ograniczenie Apple).
- **Maile magic-link** mogą trafiać do SPAM przy wysyłce z konsumenckiego SMTP —
  docelowo nadawca z uwierzytelnioną domeną (SPF/DKIM) lub provider transakcyjny.
- **Kroki = z dystansu GPS** (`steps = round(metry / 0.75)`), nie z akcelerometru
  — spójne między urządzeniami i odporne na stanie w miejscu (deadband GPS).
- **Punkty na Start** odświeżają się przy powrocie na ekran (focus/visibility).

---

## 8. Stack technologiczny

| Warstwa | Technologia |
|---|---|
| Frontend | React 19, Vite, TypeScript, Tailwind CSS, motion, react-router |
| PWA | manifest + service worker (offline, instalowalność) |
| Backend | Rust, Axum, tokio, sqlx, jsonwebtoken |
| Baza | PostgreSQL + PostGIS (Supabase) |
| Auth | JWT (aplikacja) + Supabase OTP (magic-link) |
| Storage | Supabase Storage (zdjęcia eko) |
| Hosting FE | Vercel (projekt `seasteps-app`, konto olimpia-intuflow) |
| Hosting BE | Azure App Service Linux B1 (kontener, 1 instancja) |
| CI/CD BE | GitHub Actions → ghcr → webhook CD → App Service pull `:latest` |
