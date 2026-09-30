import { Capacitor } from '@capacitor/core'
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { IconContext } from '@phosphor-icons/react'
import './index.css'
import App from './App.tsx'
import { ModeProvider } from './lib/mode'
import { ErrorBoundary } from './components/ErrorBoundary'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ErrorBoundary>
      <BrowserRouter basename={import.meta.env.BASE_URL.replace(/\/$/, '')}>
        <IconContext.Provider value={{ weight: 'fill' }}>
          <ModeProvider>
            <App />
          </ModeProvider>
        </IconContext.Provider>
      </BrowserRouter>
    </ErrorBoundary>
  </StrictMode>,
)

// PWA — rejestracja service workera (instalowalność + offline)
// W apce natywnej (Capacitor) bundle jest lokalny — SW niepotrzebny (spec 2026-10-01).
if ('serviceWorker' in navigator && !Capacitor.isNativePlatform()) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register(`${import.meta.env.BASE_URL}sw.js`).catch(() => {})
  })
}
