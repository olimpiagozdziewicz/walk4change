/**
 * Supabase client — used ONLY for magic-link auth (email OTP).
 * After Supabase establishes a session, we exchange its access token for this
 * app's own JWT via the backend (`/auth/supabase`); all data calls keep using
 * the backend token. Implicit flow keeps magic links working across devices.
 */
import { createClient } from '@supabase/supabase-js'

const url = import.meta.env.VITE_SUPABASE_URL ?? ''
const anon = import.meta.env.VITE_SUPABASE_ANON_KEY ?? ''

export function hasSupabase(): boolean {
  return url.length > 0 && anon.length > 0
}

export const supabase = createClient(url || 'https://placeholder.supabase.co', anon || 'placeholder-anon-key', {
  auth: {
    detectSessionInUrl: true,
    persistSession: true,
    autoRefreshToken: false,
    flowType: 'implicit',
  },
})
