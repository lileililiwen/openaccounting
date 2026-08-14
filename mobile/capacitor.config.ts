import type { CapacitorConfig } from '@capacitor/cli';

const config: CapacitorConfig = {
  appId: 'com.openaccounting.app',
  appName: 'OpenAccounting',
  webDir: 'www',
  server: {
    androidScheme: 'https',
    // Point the WebView at your running OpenAccounting server.
    // Override via OPENACCOUNTING_SERVER_URL env var at build time.
    url: process.env.OPENACCOUNTING_SERVER_URL ?? 'https://localhost:3000',
  },
  plugins: {
    PushNotifications: {
      presentationOptions: ['badge', 'sound', 'alert'],
    },
  },
};

export default config;
