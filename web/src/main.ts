import { UltraErrorBoundary } from 'ultra-light-js';
import { App } from './App';
import { AppLoader } from './components/app-loader/app-loader';
import { registerLeaveGuard } from './services/download-guard.service';
import { registerGlobalShortcuts } from './services/shortcuts.service';

registerLeaveGuard();
registerGlobalShortcuts();

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
