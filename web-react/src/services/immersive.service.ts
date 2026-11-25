import { registerPlugin } from '@capacitor/core';
import { isAndroidPlatform } from '@/services/server-config.service';

interface ImmersivePlugin {
    setEnabled(options: { enabled: boolean }): Promise<void>;
}

const Immersive = registerPlugin<ImmersivePlugin>('Immersive');

export function setImmersiveMode(enabled: boolean): void {
    if (!isAndroidPlatform()) return;
    Immersive.setEnabled({ enabled }).catch(() => {});
}
