const KEY = 'ss-interests'
const DEFAULT = ['Spacery nad morzem', 'Natura', 'Mindfulness', 'Eko']

export function getInterests(): string[] {
  try {
    const v = localStorage.getItem(KEY)
    return v ? (JSON.parse(v) as string[]) : DEFAULT
  } catch {
    return DEFAULT
  }
}

export function saveInterests(list: string[]) {
  localStorage.setItem(KEY, JSON.stringify(list))
}
