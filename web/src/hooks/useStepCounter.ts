import { useEffect, useRef, useState, useCallback } from 'react'

const STRIDE_M = 0.75
const ALPHA = 0.1          // low-pass filter: isolate gravity baseline
const PEAK_THRESHOLD = 1.2 // m/s² above filtered baseline → step
const MIN_STEP_MS = 250    // debounce: no two steps within 250 ms

type StepSource = 'accelerometer' | 'gps'

export interface StepCounterResult {
  steps: number
  source: StepSource
  permissionNeeded: boolean
  requestPermission: () => Promise<void>
  addMeters: (m: number) => void
  reset: () => void
}

export function useStepCounter(): StepCounterResult {
  const [steps, setSteps] = useState(0)
  const [source, setSource] = useState<StepSource>('gps')
  const [permissionNeeded, setPermissionNeeded] = useState(false)

  const accelActiveRef = useRef(false)
  const filteredRef = useRef(0)
  const lastStepAtRef = useRef(0)
  const gpsAccumRef = useRef(0)

  const onMotion = useCallback((e: DeviceMotionEvent) => {
    const a = e.accelerationIncludingGravity
    if (!a) return
    const mag = Math.sqrt((a.x ?? 0) ** 2 + (a.y ?? 0) ** 2 + (a.z ?? 0) ** 2)
    filteredRef.current = ALPHA * mag + (1 - ALPHA) * filteredRef.current
    const delta = mag - filteredRef.current
    const now = Date.now()
    if (delta > PEAK_THRESHOLD && now - lastStepAtRef.current > MIN_STEP_MS) {
      lastStepAtRef.current = now
      setSteps((s) => s + 1)
    }
  }, [])

  const startAccelerometer = useCallback(() => {
    accelActiveRef.current = true
    setSource('accelerometer')
    window.addEventListener('devicemotion', onMotion)
  }, [onMotion])

  useEffect(() => {
    if (typeof DeviceMotionEvent === 'undefined') return
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    if (typeof (DeviceMotionEvent as any).requestPermission === 'function') {
      setPermissionNeeded(true)
      return
    }
    startAccelerometer()
    return () => window.removeEventListener('devicemotion', onMotion)
  }, [onMotion, startAccelerometer])

  const requestPermission = useCallback(async () => {
    try {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const result = await (DeviceMotionEvent as any).requestPermission()
      if (result === 'granted') {
        startAccelerometer()
        setPermissionNeeded(false)
      }
    } catch {
      /* denied — stay on GPS fallback */
    }
  }, [startAccelerometer])

  const addMeters = useCallback((m: number) => {
    if (accelActiveRef.current) return
    gpsAccumRef.current += m / STRIDE_M
    setSteps(Math.round(gpsAccumRef.current))
  }, [])

  const reset = useCallback(() => {
    setSteps(0)
    gpsAccumRef.current = 0
    filteredRef.current = 0
    lastStepAtRef.current = 0
  }, [])

  return { steps, source, permissionNeeded, requestPermission, addMeters, reset }
}
