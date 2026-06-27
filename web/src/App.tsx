import { Routes, Route, Outlet } from 'react-router-dom'
import { AppShell } from './components/AppShell'
import { Login } from './screens/Login'
import { Home } from './screens/Home'
import { Walk } from './screens/Walk'
import { Community } from './screens/Community'
import { Events } from './screens/Events'
import { Profile } from './screens/Profile'
import { Eco } from './screens/Eco'
import { History } from './screens/History'
import { Partners } from './screens/Partners'
import { MagicVerify } from './screens/MagicVerify'

/** Layout apki — responsywny shell (sidebar/bottom-nav). */
function AppLayout() {
  return (
    <AppShell>
      <Outlet />
    </AppShell>
  )
}

function App() {
  return (
    <Routes>
      {/* logowanie / zakładanie konta — pełny ekran, bez shellu */}
      <Route path="/login" element={<Login />} />

      {/* aplikacja */}
      <Route element={<AppLayout />}>
        <Route path="/" element={<Home />} />
        <Route path="/walk" element={<Walk />} />
        <Route path="/auth/magic" element={<MagicVerify />} />
        <Route path="/community" element={<Community />} />
        <Route path="/events" element={<Events />} />
        <Route path="/eco" element={<Eco />} />
        <Route path="/history" element={<History />} />
        <Route path="/partners" element={<Partners />} />
        <Route path="/profile" element={<Profile />} />
        <Route path="*" element={<Home />} />
      </Route>
    </Routes>
  )
}

export default App
