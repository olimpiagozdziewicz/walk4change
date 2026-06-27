/**
 * SeaSteps — warstwa danych.
 *
 * NA TERAZ: zwraca dane mockowe (demo na hackathon).
 * PÓŹNIEJ: backend Kamila (Rust/Axum) wystawia REST + WebSocket.
 *   Wystarczy ustawić VITE_API_BASE w .env i zamienić ciała funkcji na fetch().
 *   Kształt typów poniżej jest celowo zgodny z trasami backendu:
 *   /auth, /walks, /friends, /leaderboard, /profile, /rewards.
 *
 * Mnożniki punktów (zgodne ze scoring engine backendu):
 *   spacer z kimś ×1.5, strefa natury ×3 — i one się mnożą (stackują).
 */

import { apiRequest, getToken, hasBackend, API_BASE } from './http'
import { currentUserId, setCurrentUserId } from './auth'

export { API_BASE } // '' = tryb mock (re-eksport dla zgodności)

// ── Typy (kontrakt z backendem) ───────────────────────────
export interface Profile {
  id: string
  name: string
  avatar: string // emoji lub URL
  interests: string[]
  stats: {
    walks: number
    events: number
    ecoReports: number
  }
  badges: { id: string; label: string; iconKey: string }[]
}

export interface TodayStats {
  steps: number
  points: number
  streakDays: number
  /** procent do następnej nagrody, 0–100 */
  rewardProgress: number
  natureBonusActive: boolean
  togetherBonusActive: boolean
}

export interface CommunityWalk {
  id: string
  who: string
  avatar: string
  where: string
  when: string
  vibe: string
}

export type EventType = 'cleanup' | 'planting' | 'social' | 'baltic'

export interface EventItem {
  id: string
  title: string
  type: EventType
  date: string
  place: string
  peopleCount: number
  points: number
  joined?: boolean
}

export interface Reward {
  id: string
  title: string
  kind: string
  iconKey: string
  /** 0–100 */
  progress: number
}

export interface EcoReport {
  id: string
  type: string
  description: string
  location: string
  status: 'open' | 'cleaned' | 'reported'
}

export interface LeaderboardRow {
  rank: number
  name: string
  avatar: string
  points: number
  isMe?: boolean
}

// ── Scoring (lokalny, lustro silnika backendu) ────────────
export const MULTIPLIER = { together: 1.5, nature: 3 } as const

export function computeWalkPoints(opts: {
  steps: number
  withSomeone: boolean
  inNature: boolean
}): { base: number; total: number; multiplier: number } {
  const base = Math.round(opts.steps / 20) // ~1 pkt za 20 kroków
  let multiplier = 1
  if (opts.withSomeone) multiplier *= MULTIPLIER.together
  if (opts.inNature) multiplier *= MULTIPLIER.nature
  return { base, total: Math.round(base * multiplier), multiplier }
}

// ── Mocki demo ────────────────────────────────────────────
const me: Profile = {
  id: 'me',
  name: 'Ola',
  avatar: '🌊',
  interests: ['Spacery nad morzem', 'Natura', 'Mindfulness', 'Eko'],
  stats: { walks: 23, events: 4, ecoReports: 6 },
  badges: [
    { id: 'b1', label: 'Pierwszy spacer', iconKey: 'firststep' },
    { id: 'b2', label: '7 dni z rzędu', iconKey: 'streak' },
    { id: 'b3', label: 'Strażniczka brzegu', iconKey: 'shore' },
    { id: 'b4', label: 'Sadziła drzewo', iconKey: 'tree' },
  ],
}

const today: TodayStats = {
  steps: 6842,
  points: 148,
  streakDays: 4,
  rewardProgress: 72,
  natureBonusActive: true,
  togetherBonusActive: false,
}

const communityWalks: CommunityWalk[] = [
  { id: 'c1', who: 'Bek', avatar: '🚶', where: 'Bulwar Gdynia', when: 'Dziś 17:30', vibe: 'Spokojnie, nad wodą' },
  { id: 'c2', who: 'Marta', avatar: '🧘', where: 'Park Oliwski', when: 'Jutro 9:00', vibe: 'Poranny reset' },
  { id: 'c3', who: 'Kamil', avatar: '🏃', where: 'Plaża Brzeźno', when: 'Sob 11:00', vibe: 'Żwawo + kawa' },
]

const events: EventItem[] = [
  { id: 'e1', title: 'Sprzątanie plaży Brzeźno', type: 'cleanup', date: 'Sob 28.06 • 10:00', place: 'Molo Brzeźno', peopleCount: 18, points: 120 },
  { id: 'e2', title: 'Sadzenie drzew — Trójmiejski Park', type: 'planting', date: 'Nd 29.06 • 11:00', place: 'TPK, wejście Dolina Radości', peopleCount: 9, points: 200 },
  { id: 'e3', title: 'Spacer społeczny nad Zatoką', type: 'social', date: 'Pt 27.06 • 18:00', place: 'Bulwar Nadmorski', peopleCount: 24, points: 60 },
]

const rewards: Reward[] = [
  { id: 'r1', title: 'Adopcja foki', kind: 'Cel ekologiczny', iconKey: 'seal', progress: 72 },
  { id: 'r2', title: 'Posadzenie drzewa', kind: 'Cel ekologiczny', iconKey: 'tree', progress: 45 },
  { id: 'r3', title: 'Voucher partnera', kind: 'Nagroda lokalna', iconKey: 'voucher', progress: 30 },
]

const ecoReports: EcoReport[] = [
  { id: 'x1', type: 'Śmieci na brzegu', description: 'Worek śmieci przy wejściu na plażę', location: 'Brzeźno, molo', status: 'cleaned' },
  { id: 'x2', type: 'Większe zanieczyszczenie', description: 'Rozlana substancja przy kanale', location: 'Górki Zachodnie', status: 'reported' },
]

const leaderboard: LeaderboardRow[] = [
  { rank: 1, name: 'Marta', avatar: '🧘', points: 312 },
  { rank: 2, name: 'Bek', avatar: '🚶', points: 268 },
  { rank: 3, name: 'Ola', avatar: '🌊', points: 148, isMe: true },
  { rank: 4, name: 'Kamil', avatar: '🏃', points: 134 },
]

// ── Ludzie do dopasowania (matching-lite) ─────────────────
export interface MatchPerson {
  id: string
  name: string
  avatar: string
  interests: string[]
  bio: string
  distance: string
}

export const INTEREST_OPTIONS = [
  'Spacery nad morzem',
  'Natura',
  'Mindfulness',
  'Eko',
  'Bieganie',
  'Joga',
  'Fotografia',
  'Pies',
  'Kawa i rozmowy',
  'Sprzątanie plaż',
  'Rower',
  'Medytacja',
]

const people: MatchPerson[] = [
  { id: 'p1', name: 'Marta', avatar: '🧘', interests: ['Mindfulness', 'Natura', 'Joga', 'Kawa i rozmowy'], bio: 'Poranne spacery dla resetu głowy.', distance: '1,2 km' },
  { id: 'p2', name: 'Bek', avatar: '🚶', interests: ['Spacery nad morzem', 'Eko', 'Pies', 'Sprzątanie plaż'], bio: 'Chodzę z psem, sprzątam przy okazji.', distance: '800 m' },
  { id: 'p3', name: 'Igor', avatar: '📷', interests: ['Fotografia', 'Natura', 'Spacery nad morzem'], bio: 'Łapię światło o wschodzie nad Zatoką.', distance: '2,4 km' },
  { id: 'p4', name: 'Hania', avatar: '🌿', interests: ['Eko', 'Mindfulness', 'Medytacja', 'Natura'], bio: 'Wolne tempo, dużo zieleni.', distance: '600 m' },
]

// ── Lokalni partnerzy / sponsorzy ─────────────────────────
export type SponsorIconKey = 'boat' | 'sup' | 'bike' | 'coffee' | 'sail' | 'food' | 'icecream'

export interface Sponsor {
  id: string
  name: string
  category: string
  offer: string
  pointsCost: number
  iconKey: SponsorIconKey
  place: string
}

const sponsors: Sponsor[] = [
  { id: 'sp1', name: 'Kajaki Zatoka', category: 'Wypożyczalnia kajaków', offer: '−20% na spływ', pointsCost: 120, iconKey: 'boat', place: 'Marina Gdynia' },
  { id: 'sp2', name: 'SUP Sopot', category: 'Deski SUP', offer: '1h gratis przy 2h', pointsCost: 150, iconKey: 'sup', place: 'Molo Sopot' },
  { id: 'sp3', name: 'Rowery Nadmorskie', category: 'Wypożyczalnia rowerów', offer: '−15% na dzień', pointsCost: 80, iconKey: 'bike', place: 'Bulwar Nadmorski' },
  { id: 'sp4', name: 'Przystań Kawa', category: 'Kawiarnia nad wodą', offer: 'Kawa −50%', pointsCost: 60, iconKey: 'coffee', place: 'Brzeźno' },
  { id: 'sp5', name: 'Szkoła Żeglarstwa', category: 'Rejsy i lekcje', offer: 'Lekcja próbna −30%', pointsCost: 200, iconKey: 'sail', place: 'Górki Zachodnie' },
  { id: 'sp6', name: 'Bar Przystań', category: 'Smażalnia ryb nad wodą', offer: '−15% na obiad', pointsCost: 100, iconKey: 'food', place: 'Sopot, molo' },
  { id: 'sp7', name: 'Lody Bałtyk', category: 'Lodziarnia rzemieślnicza', offer: '2 gałki w cenie 1', pointsCost: 50, iconKey: 'icecream', place: 'Gdynia, bulwar' },
]

// ── Wariant korporacyjny (B2B) ────────────────────────────
export interface TeamToday {
  company: string
  teamName: string
  members: number
  steps: number
  points: number
  rewardTitle: string
  rewardProgress: number
  teamMultiplier: number
}

export interface TeamRow {
  rank: number
  team: string
  points: number
  members: number
  isMine?: boolean
}

const teamToday: TeamToday = {
  company: 'Northwind',
  teamName: 'Zespół Marketing',
  members: 8,
  steps: 48210,
  points: 1240,
  rewardTitle: 'Dzień wolny dla zespołu',
  rewardProgress: 64,
  teamMultiplier: 2,
}

const teamLeaderboard: TeamRow[] = [
  { rank: 1, team: 'Sprzedaż', points: 1880, members: 11 },
  { rank: 2, team: 'Marketing', points: 1240, members: 8, isMine: true },
  { rank: 3, team: 'IT', points: 1120, members: 14 },
  { rank: 4, team: 'HR', points: 760, members: 5 },
]

const corporateEvents: EventItem[] = [
  { id: 'ce1', title: 'Spacer integracyjny działu', type: 'social', date: 'Śr 02.07 • 15:00', place: 'Bulwar Nadmorski', peopleCount: 8, points: 160 },
  { id: 'ce2', title: 'Firmowe sprzątanie plaży (CSR)', type: 'cleanup', date: 'Sob 05.07 • 10:00', place: 'Plaża Stogi', peopleCount: 22, points: 300 },
  { id: 'ce3', title: 'Sadzenie drzew — las firmowy', type: 'planting', date: 'Nd 13.07 • 11:00', place: 'TPK Dolina Radości', peopleCount: 16, points: 400 },
]

const teamRewards: Reward[] = [
  { id: 'tr1', title: 'Dzień wolny dla zespołu', kind: 'Nagroda firmowa', iconKey: 'dayoff', progress: 64 },
  { id: 'tr2', title: 'Budżet na integrację', kind: 'Nagroda firmowa', iconKey: 'integration', progress: 40 },
  { id: 'tr3', title: 'Dzień wellbeing', kind: 'Nagroda firmowa', iconKey: 'wellbeing', progress: 28 },
]

// symulacja opóźnienia sieci, żeby UI był „prawdziwy"
const wait = <T>(data: T, ms = 150): Promise<T> =>
  new Promise((res) => setTimeout(() => res(data), ms))

// ── Realny backend (gdy VITE_API_BASE ustawione i mamy token) ─────────────
// Tylko część ekranów ma odpowiednik w backendzie (profil, nagrody, ranking).
// Reszta (eventy, eko, sponsorzy, zespoły, dopasowania, „dziś") to nadal mock.

/** Czy używać realnego backendu dla danego wywołania. */
const live = (): boolean => hasBackend() && !!getToken()

interface BackendProfile {
  id: string
  email: string
  display_name: string
  avatar_url: string | null
  bio: string | null
  interests: string[]
  created_at: string
}

interface BackendReward {
  id: string
  title: string
  description: string | null
  cost_points: string
  partner_name: string | null
  type: string
  stock: number | null
  image_url: string | null
}

interface BackendLeaderRow {
  user_id: string
  display_name: string
  total_points: string
}

function mapProfile(p: BackendProfile): Profile {
  return {
    id: p.id,
    name: p.display_name,
    // backend trzyma URL/null; UI używa emoji — fallback gdy brak URL-a
    avatar: p.avatar_url ?? '🌊',
    interests: p.interests ?? [],
    stats: { walks: 0, events: 0, ecoReports: 0 },
    badges: [],
  }
}

function mapReward(r: BackendReward): Reward {
  return {
    id: r.id,
    title: r.title,
    kind: r.partner_name ?? r.type,
    iconKey: 'voucher',
    progress: 0,
  }
}

function mapLeaderRow(row: BackendLeaderRow, index: number, myId: string | null): LeaderboardRow {
  return {
    rank: index + 1,
    name: row.display_name,
    avatar: '🚶',
    points: Math.round(parseFloat(row.total_points)), // rust_decimal => string
    isMe: myId != null && row.user_id === myId,
  }
}

async function fetchProfile(): Promise<Profile> {
  const res = await apiRequest<BackendProfile>('/me')
  const p = res.data
  if (!p) throw new Error('Brak danych profilu')
  setCurrentUserId(p.id)
  return mapProfile(p)
}

async function fetchRewards(): Promise<Reward[]> {
  const res = await apiRequest<BackendReward[]>('/rewards')
  return (res.data ?? []).map(mapReward)
}

async function fetchLeaderboard(): Promise<LeaderboardRow[]> {
  const res = await apiRequest<BackendLeaderRow[]>('/leaderboard?per_page=20')
  const myId = currentUserId()
  return (res.data ?? []).map((row, i) => mapLeaderRow(row, i, myId))
}

/**
 * Realny fetch z miękkim fallbackiem na mock. Dzięki temu apka działa nawet,
 * gdy backend chwilowo nie odpowiada (demo na hackathonie nie pada).
 */
async function liveOrMock<T>(fetcher: () => Promise<T>, mock: T, label: string): Promise<T> {
  if (!live()) return wait(mock)
  try {
    return await fetcher()
  } catch (err) {
    console.warn(`[api] ${label}: backend niedostępny, używam mocka`, err)
    return mock
  }
}

export const api = {
  getProfile: () => liveOrMock(fetchProfile, me, 'getProfile'),
  getToday: () => wait(today),
  getCommunityWalks: () => wait(communityWalks),
  getEvents: () => wait(events),
  getRewards: () => liveOrMock(fetchRewards, rewards, 'getRewards'),
  getEcoReports: () => wait(ecoReports),
  getLeaderboard: () => liveOrMock(fetchLeaderboard, leaderboard, 'getLeaderboard'),
  getMatches: () => wait(people),
  getSponsors: () => wait(sponsors),
  // wariant korporacyjny (brak w backendzie — zawsze mock)
  getTeamToday: () => wait(teamToday),
  getTeamLeaderboard: () => wait(teamLeaderboard),
  getCorporateEvents: () => wait(corporateEvents),
  getTeamRewards: () => wait(teamRewards),
}
