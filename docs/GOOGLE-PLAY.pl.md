# SeaSteps w Google Play — instrukcja krok po kroku (dla Olimpii)

Stan techniczny: apka Android jest GOTOWA. Podpisany plik `SeaSteps.aab` buduje
się jednym kliknięciem w GitHub Actions (workflow `android`). Ta instrukcja
prowadzi przez część, którą może zrobić tylko właścicielka konta.

## Krok 0 — pobierz plik AAB

1. Wejdź: https://github.com/olimpiagozdziewicz/walk4change/actions/workflows/android.yml
2. Kliknij najnowszy zielony run → sekcja **Artifacts** → pobierz **SeaSteps-aab**
   (w środku `SeaSteps.aab`).
3. Świeży build w każdej chwili: przycisk **Run workflow** (gałąź `main`) i po
   ~3 min artifact jest gotowy.

## Krok 1 — konto dewelopera Google Play (25 USD, jednorazowo)

1. Wejdź na https://play.google.com/console/signup i zaloguj się kontem Google,
   które ma być WŁAŚCICIELEM aplikacji (najlepiej admin@seasteps.pl, nie prywatny gmail).
2. Wybierz typ konta **Osoba prywatna** (spółki jeszcze nie ma — po założeniu
   spółki aplikację można przenieść na konto firmowe).
3. Opłata 25 USD (jednorazowa, karta). Google może poprosić o weryfikację
   tożsamości (dokument) — standard, trwa 1–3 dni.

## Krok 2 — utwórz aplikację

1. Play Console → **Utwórz aplikację**: nazwa `SeaSteps`, język polski,
   typ **Aplikacja**, bezpłatna.
2. **Play App Signing** zostaw WŁĄCZONE (domyślne) — Google zarządza kluczem
   publikacji; nasz klucz z GitHuba jest tylko „upload key". Jego utrata jest
   odwracalna przez support.

## Krok 3 — wypełnij kartę aplikacji (sekcja „Rozwój aplikacji → Karta")

- Krótki opis (80 znaków): `Spacery, które robią dobrze Tobie i naturze — punkty za ruch, razem i w naturze.`
- Pełny opis: na bazie landing seasteps.pl (mogę przygotować na życzenie).
- Grafiki: ikona 512×512 i feature graphic 1024×500 (mam wygenerować — daj znać),
  minimum 2 screenshoty telefonu (zrzuty z apki na Twoim telefonie wystarczą).
- **Polityka prywatności (wymagana):** `https://seasteps.pl/privacy.html` ✅ (już opisuje GPS).

## Krok 4 — ankiety (sekcja „Zawartość aplikacji")

1. **Data safety (Bezpieczeństwo danych)** — deklarujemy zgodnie z prawdą:
   - Lokalizacja (dokładna): zbierana, NIE udostępniana podmiotom trzecim,
     cel: funkcje aplikacji; zbierana tylko podczas spaceru (użytkownik może
     usunąć dane — jest przycisk „Usuń konto").
   - Adres e-mail + nazwa: zbierane, cel: zarządzanie kontem.
   - Wiadomości w aplikacji: zbierane, cel: funkcje aplikacji.
   - Dane są szyfrowane w tranzycie: TAK. Można poprosić o usunięcie: TAK.
2. **Content rating** — ankieta IARC: aplikacja bez przemocy/hazardu, ale MA
   czat/interakcje użytkowników → zaznacz „users can communicate". Wyjdzie
   ocena w okolicy PEGI 3–12.
3. **Docelowi odbiorcy:** 16+ (zgodnie z regulaminem). NIE zaznaczaj „aplikacja
   dla dzieci".
4. **Aplikacja rządowa / finansowa / zdrowotna:** nie.

## Krok 5 — test zamknięty (wymóg nowych kont prywatnych!)

Nowe prywatne konta deweloperskie MUSZĄ przed publiczną publikacją przejść
**test zamknięty: min. 12 testerów aktywnych przez 14 dni** (zalecane ~20).
To współgra z naszym miękkim startem:

1. **Testowanie → Test zamknięty → Utwórz ścieżkę** (track „SeaSteps beta").
2. **Prześlij `SeaSteps.aab`** (z Kroku 0).
3. Dodaj listę e-maili testerów (znajomi, rodzina, społeczność bałtycka —
   zbierzemy razem ~20 adresów Google).
4. Wyślij testerom link do dołączenia (Console go wygeneruje).
5. Po 14 dniach aktywnego testu Console odblokuje wniosek o **produkcję**.

## Krok 6 — po testach: produkcja

„Wersje → Produkcja → Utwórz wersję" → ten sam AAB (lub świeży z Actions) →
przegląd Google (zwykle 1–7 dni) → publiczna publikacja.

## Ważne pliki i sekrety (dla agentów)

- Upload keystore: `BIZNES/dane/seasteps-android-secrets-upload-keystore.p12`
  (hasło w `api-secrets.local.md`); kopie w GitHub Secrets
  (`ANDROID_KEYSTORE_B64`, `ANDROID_KEYSTORE_PASS`).
- Wersjonowanie: `web/android/app/build.gradle` → `versionCode` (podbijać przy
  każdym uploadzie do Play) i `versionName`.
- Zmienne builda: GitHub vars `VITE_API_BASE`, `VITE_SUPABASE_URL`,
  `VITE_SUPABASE_ANON_KEY` (wartości publiczne).
- Backend CORS ma już `https://localhost` (origin natywnej apki) — env
  `CORS_ALLOWED_ORIGINS` na Azure.

## App Links (magic link w apce) — odcisk App signing key

Magic link otwiera się w apce dzięki App Links (`autoVerify`, path `/app/auth/magic`)
i plikowi `.well-known/assetlinks.json` na seasteps.pl (kopiowany w `build-site.sh`).
Plik zawiera odcisk SHA-256 klucza **upload** (wypisuje go CI, artefakt `assetlinks`).

**Po utworzeniu apki w Play Console (App signing) trzeba dopisać DRUGI odcisk** —
„App signing key certificate" z Konsoli (Konfiguracja → Integralność aplikacji →
Podpisywanie aplikacji) — do tablicy `sha256_cert_fingerprints` w `assetlinks.json`
i wdrożyć stronę. Sklep podpisuje instalacje kluczem Google, więc bez tego
odcisku App Links NIE zweryfikują się na zainstalowanej z Play apce (link otworzy się
w przeglądarce). Weryfikacja: `adb shell pm get-app-links pl.seasteps.app`.

## Czego w tej wersji celowo NIE ma (następne iteracje)

- Powiadomienia push (FCM), Play Integrity API.
- iOS — iPhone'y obsługuje PWA na seasteps.pl („Dodaj do ekranu głównego").
