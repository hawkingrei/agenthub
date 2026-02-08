import { api } from "./api";
import { urlBase64ToUint8Array } from "./webauthn";

export async function ensurePushSubscription(token: string) {
  if (!("serviceWorker" in navigator)) return;
  if (!("PushManager" in window)) return;
  let registration: ServiceWorkerRegistration;
  try {
    registration =
      (await navigator.serviceWorker.getRegistration()) ??
      (await navigator.serviceWorker.register("/sw.js"));
    registration = await navigator.serviceWorker.ready;
  } catch {
    return;
  }
  if (!registration.pushManager) return;
  let sub = await registration.pushManager.getSubscription();
  if (!sub) {
    let vapid: { public_key: string };
    try {
      vapid = await api.getVapidPublicKey();
    } catch {
      return;
    }
    const key = urlBase64ToUint8Array(vapid.public_key);
    try {
      sub = await registration.pushManager.subscribe({
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
