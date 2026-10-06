import { useCallback, useRef, useState } from 'react'

const STRIDE_M = 0.75 // meters per step (average adult stride)

type StepSource = 'gps' | 'sensor'

export interface StepCounterResult {
  steps: number
  source: StepSource
  permissionNeeded: boolean
  requestPermission: () => Promise<void>
  addMeters: (m: number) => void
  /** Przyrost kroków z czujnika telefonu — od pierwszego wywołania kroki liczy czujnik. */
  addSteps: (n: number) => void
  reset: () => void
}

/**
 * Step counter: czujnik kroków telefonu, gdy jest (apka Android, spec
 * 2026-10-06), inaczej kroki z dystansu GPS.
 *
 * Fallback GPS (przeglądarka, iPhone, brak zgody):
 *
 * Steps are derived purely from server-credited GPS distance
 * (`steps = round(meters / stride)`). This is robust across devices:
 * Android browsers frequently expose `devicemotion` but never trip a reliable
 * step peak (or the sensor is absent), which previously left the accelerometer
 * "active" while counting zero steps AND suppressing the GPS fallback.
 *
 * Fallback NIE jest odporny na bezruch: dryf GPS w budynku (skoki 15–38 m)
 * przechodzi przez filtry serwera (sesja 05.10: 131 m przy przesunięciu 1 m).
 * Dlatego w apce kroki i metry pilnuje czujnik — patrz `lib/stepGate.ts`.
 */
export function useStepCounter(): StepCounterResult {
  const [steps, setSteps] = useState(0)
  const [source, setSource] = useState<StepSource>('gps')
  const gpsAccumRef = useRef(0)
  const sensorRef = useRef(false)

  const addMeters = useCallback((m: number) => {
    // Gdy liczy czujnik, metry już nie zamieniają się w kroki. Wyjątek: suma
    // z wznowionego spaceru dodana przed startem czujnika — zostaje bazą.
    if (m <= 0 || sensorRef.current) return
    gpsAccumRef.current += m / STRIDE_M
    setSteps(Math.round(gpsAccumRef.current))
  }, [])

  const addSteps = useCallback((n: number) => {
    if (!sensorRef.current) {
      sensorRef.current = true
      setSource('sensor')
    }
    if (n <= 0) return
    gpsAccumRef.current += n
    setSteps(Math.round(gpsAccumRef.current))
  }, [])

  const reset = useCallback(() => {
    setSteps(0)
    setSource('gps')
    gpsAccumRef.current = 0
    sensorRef.current = false
  }, [])

  // Accelerometer permission is no longer used; keep the interface stable.
  const requestPermission = useCallback(async () => {}, [])

  return { steps, source, permissionNeeded: false, requestPermission, addMeters, addSteps, reset }
}
