import { UltraErrorBoundary } from 'ultra-light-js';
import { App } from './App';
import { AppLoader } from './components/app-loader/app-loader';
import { registerDesktopCloseGuard } from './services/close-guard.service';

declare global {
    interface Window {
        versions?: {
            chrome: string;
            node: string;
            platform: string;
        };
        desktop?: {
            pickLibraryFolder(): Promise<string | null>;
            downloadComic(request: {
                url: string;
                name: string;
                headers?: Record<string, string>;
            }): Promise<string | null>;
            toggleFullscreen(): Promise<void>;
            minimizeWindow(): Promise<void>;
            toggleMaximizeWindow(): Promise<void>;
            closeWindow(): Promise<void>;
            isWindowMaximized(): Promise<boolean>;
            onMaximizedChange(callback: (isMaximized: boolean) => void): () => void;
            confirmClose(): Promise<void>;
            cancelClose(): Promise<void>;
            onCloseRequested(callback: (activeDownloads: number) => void): () => void;
        };
    }
}

registerDesktopCloseGuard();

const platform = window.versions?.platform;
if (platform) document.documentElement.dataset.platform = platform;

const $app = document.getElementById('root');

$app?.appendChild(
    UltraErrorBoundary({
        factories: App,
        fallback: (e) =>
            AppLoader({
                message: e instanceof Error ? `${e.stack}` : 'There was an error',
            }),
    }),
);
