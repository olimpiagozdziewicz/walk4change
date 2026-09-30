import { Capacitor } from '@capacitor/core'

/** Publiczny adres strony; w natywnej apce bundle nie zawiera plików strony (np. privacy.html). */
export const SITE_ORIGIN = 'https://seasteps.pl'

/** siteUrl('/privacy.html'): w apce natywnej absolutny URL (otwiera się w przeglądarce systemowej), na webie bez zmian. */
export function siteUrl(path: string): string {
  return Capacitor.isNativePlatform() ? `${SITE_ORIGIN}${path}` : path
}
