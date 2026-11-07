// The service worker the identity fixture registers. It does nothing but exist:
// a registration that another identity can see is the link.
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()));
