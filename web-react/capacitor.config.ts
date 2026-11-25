import type { CapacitorConfig } from '@capacitor/cli';

const config: CapacitorConfig = {
  appId: 'com.amin.BetterRackweb',
  appName: 'BetterRack',
  webDir: 'dist',
  server: {
    androidScheme: 'http',
    allowNavigation: ['*']
  }
};

export default config;
