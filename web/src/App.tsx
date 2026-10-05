import { useEffect } from 'react'
import { Capacitor } from '@capacitor/core'
import { Routes, Route, Outlet, useLocation, useNavigate, Navigate } from 'react-router-dom'
import { initAppLinks } from './lib/appLinks'
import { hasPendingJoin, loadActiveWalk, normalizeJoinCode, setPendingJoin } from './lib/activeWalk'
import { isAuthed, setAuthed } from './lib/auth'
import { getToken } from './lib/http'
import { AppShell } from './components/AppShell'
import { Login } from './screens/Login'
import { Home } from './screens/Home'
import { Walk } from './screens/Walk'
import { Community } from './screens/Community'
import { Chat } from './screens/Chat'
import { Events } from './screens/Events'
import { Profile } from './screens/Profile'
import { Eco } from './screens/Eco'
import { History } from './screens/History'
import { Partners } from './screens/Partners'
import { MagicVerify } from './screens/MagicVerify'
import { VerifyEmail } from './screens/VerifyEmail'
import { InstallModal } from './components/InstallModal'

// Link z QR (…/walk?join=KOD) — kod zapamiętujemy synchronicznie przy starcie,
// zanim RequireAuth przekieruje niezalogowanego na /login i zgubi query.
;(() => {
  try {
    const u = new URL(window.location.href)
    const code = normalizeJoinCode(u.searchParams.get('join'))
    if (code && u.pathname.replace(/\/$/, '').endsWith('/walk')) setPendingJoin(code)
  } catch { /* ignore */ }
})()

/** Aktywny spacer albo kod z QR → ekran Spacer (też po zalogowaniu). */
function WalkRedirect() {
  const { pathname } = useLocation()
  const navigate = useNavigate()
  useEffect(() => {
    if (pathname === '/walk' || !isAuthed() || !getToken()) return
    if (hasPendingJoin() || loadActiveWalk()) navigate('/walk', { replace: true })
  }, [pathname, navigate])
  return null
}

function ScrollToTop() {
  const { pathname } = useLocation()
  useEffect(() => { window.scrollTo(0, 0) }, [pathname])
  return null
}

/** Guard: kick stale demo sessions (authed flag set but no real JWT). */
function RequireAuth() {
  if (isAuthed() && !getToken()) {
    setAuthed(false)
    return <Navigate to="/login" replace />
  }
  if (!isAuthed()) return <Navigate to="/login" replace />
  return <Outlet />
}

/** Layout apki — responsywny shell (sidebar/bottom-nav). */
function AppLayout() {
  return (
    <AppShell>
      <Outlet />
    </AppShell>
  )
}

function App() {
  const navigate = useNavigate()
  useEffect(() => initAppLinks((to) => navigate(to)), [navigate])
  return (
    <>
    <ScrollToTop />
    <WalkRedirect />
    <Routes>
      {/* logowanie / zakładanie konta — pełny ekran, bez shellu */}
      <Route path="/login" element={<Login />} />
      <Route path="/auth/magic" element={<MagicVerify />} />
      {/* potwierdzenie e-maila — publiczne (link można otworzyć na innym urządzeniu) */}
      <Route path="/auth/verify-email" element={<VerifyEmail />} />

      {/* aplikacja — wymaga zalogowania */}
      <Route element={<RequireAuth />}>
      <Route element={<AppLayout />}>
        <Route path="/" element={<Home />} />
        <Route path="/walk" element={<Walk />} />
        <Route path="/community" element={<Community />} />
        <Route path="/chat/:userId" element={<Chat />} />
        <Route path="/events" element={<Events />} />
        <Route path="/eco" element={<Eco />} />
        <Route path="/history" element={<History />} />
        <Route path="/partners" element={<Partners />} />
        <Route path="/profile" element={<Profile />} />
        <Route path="*" element={<Home />} />
      </Route>
      </Route>
    </Routes>
    {!Capacitor.isNativePlatform() && <InstallModal />}
    </>
  )
}

export default App
