// SeaSteps — prosty service worker (instalowalność PWA + offline fallback)
const CACHE = 'seasteps-v1'
const SHELL = ['/', '/index.html', '/manifest.webmanifest', '/favicon.svg', '/pwa-192.png', '/pwa-512.png']

self.addEventListener('install', (e) => {
  e.waitUntil(caches.open(CACHE).then((c) => c.addAll(SHELL)).catch(() => {}))
  self.skipWaiting()
})

self.addEventListener('activate', (e) => {
  e.waitUntil(
    caches.keys().then((keys) => Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k)))),
  )
  self.clients.claim()
})

self.addEventListener('fetch', (e) => {
  const req = e.request
  if (req.method !== 'GET') return
  // nawigacje: sieć, a offline -> cache (SPA)
  if (req.mode === 'navigate') {
    e.respondWith(fetch(req).catch(() => caches.match('/index.html').then((r) => r || caches.match('/'))))
    return
  }
  // reszta: cache-first z dociąganiem
  e.respondWith(
    caches.match(req).then((cached) => cached || fetch(req).then((res) => {
      const copy = res.clone()
      caches.open(CACHE).then((c) => c.put(req, copy)).catch(() => {})
      return res
    }).catch(() => cached)),
  )
})
