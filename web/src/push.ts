import { api } from "./api";
import { urlBase64ToUint8Array } from "./webauthn";

export async function ensureServiceWorkerRegistration(): Promise<ServiceWorkerRegistration | null> {
  if (!("serviceWorker" in navigator)) return null;
  try {
    const existing = await navigator.serviceWorker.getRegistration();
    if (existing) {
      return existing;
    }
    return await navigator.serviceWorker.register("/sw.js");
  } catch {
    return null;
  }
}

export async function ensurePushSubscription(token: string) {
  if (!("PushManager" in window)) return;
  const registration = await ensureServiceWorkerRegistration();
  if (!registration) {
    return;
  }
  let readyRegistration: ServiceWorkerRegistration;
  try {
    readyRegistration = await navigator.serviceWorker.ready;
  } catch {
    return;
  }
  if (!readyRegistration.pushManager) return;
  let sub = await readyRegistration.pushManager.getSubscription();
  if (!sub) {
    let vapid: { public_key: string };
    try {
      vapid = await api.getVapidPublicKey();
    } catch {
      return;
    }
    const keyBytes = urlBase64ToUint8Array(vapid.public_key);
    const key = new Uint8Array(keyBytes);
    try {
      sub = await readyRegistration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: key,
      });
    } catch {
      return;
    }
  }
  try {
    await api.subscribePush(token, sub.toJSON());
  } catch {
    return;
  }
}
