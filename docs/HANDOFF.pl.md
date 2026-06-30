# SeaSteps (walk4change) — dokumentacja przekazania

Kompletny opis działania aplikacji: frontend, backend, baza danych, wdrożenia i
lokalne uruchomienie. Sekrety (hasła, klucze, dane serwera domowego) **nie** są
tu zawarte — trzymaj je w `backend/deploy/.env.homelab` (gitignored) i w panelu
Vercel.

---

## 1. Przegląd architektury

```
  Przeglądarka (PWA)
        │  HTTPS
        ▼
  seasteps.pl  ─────────────────►  Vercel (statyczny frontend)
        │                            • landing  /
        │                            • aplikacja /app/  (React + Vite)
        │
        │  REST + WebSocket (HTTPS/WSS)
        ▼
  walk4change.<domena>
        │  Cloudflare Tunnel
        ▼
  Caddy (edge) ──► Traefik (IngressRoute) ──► Backend (Rust/Axum) w k3s :8080
        │                                            │  pojedynczy Pod
        │                                            ▼
        └──────────────►  Supabase  ◄────────  PostgreSQL (PostGIS)
                          • Auth (magic-link)        + Storage (zdjęcia eko)
```

Trzy niezależne elementy:

1. **Frontend** — React/Vite PWA, hostowany na Vercel (`seasteps.pl`).
2. **Backend** — Rust/Axum REST + WebSocket, uruchomiony w **Kubernetes (k3s)**,
   wdrażany przez **ArgoCD** (GitOps); ruch: Cloudflare → Caddy → Traefik → Pod.
3. **Supabase** — PostgreSQL (z PostGIS), Auth (magic-link) i Storage (zdjęcia).

Frontend i backend są w jednym repo: `h4cstolik3/walk4change`. Manifesty k8s
(Helm) są w osobnym repo: `kamilandrzejrybacki-inc/helm`, `charts/walk4change`.

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
└── deploy/homelab.sh     # skrypt wdrożeniowy (Docker + Cloudflare Tunnel)
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

W `backend/deploy/.env.homelab` (gitignored):

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

- **PostgreSQL** — główna baza backendu (przez session pooler).
- **Auth** — magic-link / OTP (frontend używa `supabase-js` tylko do tego;
  potem wymiana na JWT aplikacji).
- **Storage** — bucket **`eco-photos`** (publiczny). Zdjęcia eko ładowane są
  **bezpośrednio z przeglądarki** do Storage (API ma limit body 64 KiB, więc
  zdjęcia nie przechodzą przez backend — w bazie trzymamy tylko URL-e).

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

### Frontend → Vercel (`seasteps.pl`)

Build łączy landing + aplikację w jeden katalog `site/`:

```bash
# z katalogu głównego repo, z ustawionymi zmiennymi VITE_*:
bash scripts/build-site.sh         # buduje web/ (base=/app/) i składa site/
vercel deploy site --prod          # wdraża na projekt "seasteps" → seasteps.pl
```

`site/` zawiera: landing `index.html` w `/`, aplikację w `/app/`,
`privacy.html` (Polityka Prywatności) i `vercel.json` (rewrites dla SPA).

### Backend → Kubernetes (k3s) przez ArgoCD — automatyczne CI/CD

Backend działa w klastrze **k3s** i jest wdrażany w modelu **GitOps**. Nie
wdrażasz go ręcznie — wystarczy wypchnąć kod na `main`:

```
push (backend/**) → main
        │
        ▼
GitHub Actions  (.github/workflows/docker.yml)
   1. build obrazu Docker (backend/Dockerfile)
   2. push do ghcr.io/h4cstolik3/walk4change-api:<short-sha>  (+ :latest)
   3. bump tagu obrazu w  kamilandrzejrybacki-inc/helm → charts/walk4change/values.yaml
        │
        ▼
ArgoCD  (aplikacja "walk4change", auto-sync: prune + selfHeal)
   wykrywa zmianę values.yaml → synchronizuje → rolling update Poda na k3s
```

Elementy:

- **Obraz** — `ghcr.io/h4cstolik3/walk4change-api`, tag = krótki SHA commita.
- **Helm chart** — repo `kamilandrzejrybacki-inc/helm`, ścieżka `charts/walk4change`
  (Deployment, Service, PDB, Traefik IngressRoute + Middleware + WS ServersTransport,
  Namespace). `image.tag` w `values.yaml` jest aktualizowany automatycznie przez CI.
- **ArgoCD** — aplikacja `walk4change` (namespace docelowy `walk4change`),
  `syncPolicy.automated` (prune + selfHeal). Po bumpie tagu sync dzieje się sam.
- **Sekrety** (DATABASE_URL, JWT_SECRET, SMTP_USER, SMTP_PASS) — przez `SopsSecret`
  `walk4change-secrets` (envFrom). Jawne, niewrażliwe env — w `values.yaml`.
- **Ruch publiczny** — Cloudflare Tunnel → Caddy → Traefik IngressRoute → Pod.
- **Migracje** bazy aplikują się przy starcie Poda (`sqlx::migrate!()`).
- ⚠️ **`replicas: 1`** wymuszone (hub WS w pamięci).

**Wymagany sekret CI** (jednorazowo): w ustawieniach repo `h4cstolik3/walk4change`
dodaj sekret `HELM_DEPLOY_TOKEN` — PAT z prawem zapisu do repo
`kamilandrzejrybacki-inc/helm` (krok CI klonuje je i wypycha bump tagu).

Ręczny re-deploy bez zmian w kodzie: uruchom workflow `docker` przez
`workflow_dispatch` (zakładka Actions) lub `gh workflow run docker.yml --ref main`.

> Lokalny/awaryjny wariant (poza k8s): `backend/deploy/homelab.sh` uruchamia
> backend jako pojedynczy kontener Docker na `:8080`. Używany do dev/demo, nie do
> produkcji (produkcja = k3s + ArgoCD powyżej).

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
| Hosting FE | Vercel |
| Hosting BE | Kubernetes (k3s) + ArgoCD (GitOps); ruch Cloudflare → Caddy → Traefik |
| CI/CD BE | GitHub Actions → ghcr → bump Helm tag → ArgoCD auto-sync |
