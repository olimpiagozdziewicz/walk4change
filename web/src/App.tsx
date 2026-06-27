import { Routes, Route } from 'react-router-dom'
import { AppShell } from './components/AppShell'
import { Home } from './screens/Home'
import { Walk } from './screens/Walk'
import { Community } from './screens/Community'
import { Events } from './screens/Events'
import { Profile } from './screens/Profile'
import { Eco } from './screens/Eco'
import { History } from './screens/History'
import { Partners } from './screens/Partners'

/** Apka otwiera się od razu w środku (logowanie jest na landingu, nie tutaj). */
function App() {
  return (
    <AppShell>
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/walk" element={<Walk />} />
        <Route path="/community" element={<Community />} />
        <Route path="/events" element={<Events />} />
        <Route path="/eco" element={<Eco />} />
        <Route path="/history" element={<History />} />
        <Route path="/partners" element={<Partners />} />
        <Route path="/profile" element={<Profile />} />
        <Route path="*" element={<Home />} />
      </Routes>
    </AppShell>
  )
}

export default App
