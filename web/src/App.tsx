import { Routes, Route } from 'react-router-dom'
import { PhoneFrame } from './components/PhoneFrame'
import { Home } from './screens/Home'
import { Walk } from './screens/Walk'
import { Community } from './screens/Community'
import { Events } from './screens/Events'
import { Profile } from './screens/Profile'
import { Eco } from './screens/Eco'
import { History } from './screens/History'

function App() {
  return (
    <PhoneFrame>
      <Routes>
        <Route path="/" element={<Home />} />
        <Route path="/walk" element={<Walk />} />
        <Route path="/community" element={<Community />} />
        <Route path="/events" element={<Events />} />
        <Route path="/eco" element={<Eco />} />
        <Route path="/history" element={<History />} />
        <Route path="/profile" element={<Profile />} />
        <Route path="*" element={<Home />} />
      </Routes>
    </PhoneFrame>
  )
}

export default App
