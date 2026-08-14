import { PushNotifications } from '@capacitor/push-notifications';
import { Capacitor } from '@capacitor/core';

/**
 * Bootstrap the OpenAccounting Capacitor shell.
 *
 * Registers for push notifications on native platforms and
 * posts the device token to the server on successful registration.
 */
document.addEventListener('DOMContentLoaded', async () => {
  if (!Capacitor.isNativePlatform()) {
    return;
  }

  const perm = await PushNotifications.requestPermissions();
  if (perm.receive === 'granted') {
    await PushNotifications.register();
  }
});

PushNotifications.addListener('registration', async ({ value }) => {
  const platform = Capacitor.getPlatform(); // "ios" | "android"
  await fetch('/devices/register', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify({ token: value, platform }),
    credentials: 'include',
  });
});

PushNotifications.addListener('pushNotificationReceived', (notification) => {
  const url = (notification.data as Record<string, string>)?.url;
  if (url) {
    window.location.href = url;
  }
});

PushNotifications.addListener('pushNotificationActionPerformed', (action) => {
  const url = (action.notification.data as Record<string, string>)?.url;
  if (url) {
    window.location.href = url;
  }
});
