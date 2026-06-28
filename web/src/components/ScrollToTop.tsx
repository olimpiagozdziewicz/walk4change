import { useEffect } from 'react'
import { useLocation } from 'react-router-dom'

/** Przewija na samą górę przy każdej zmianie trasy (np. po zalogowaniu). */
export function ScrollToTop() {
  const { pathname } = useLocation()
  useEffect(() => {
    window.scrollTo(0, 0)
    document.documentElement.scrollTop = 0
  }, [pathname])
  return null
}
