import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { BrowserRouter } from 'react-router-dom'
import { IconContext } from '@phosphor-icons/react'
import './index.css'
import App from './App.tsx'
import { ModeProvider } from './lib/mode'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <BrowserRouter>
      <IconContext.Provider value={{ weight: 'fill' }}>
        <ModeProvider>
          <App />
        </ModeProvider>
      </IconContext.Provider>
    </BrowserRouter>
  </StrictMode>,
)

// PWA — rejestracja service workera (instalowalność + offline)
if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/sw.js').catch(() => {})
  })
}
