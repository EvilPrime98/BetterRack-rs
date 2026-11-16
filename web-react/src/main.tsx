import { createRoot } from 'react-dom/client';
import './main.css';
import { Providers } from './Providers';
import { ErrorBoundary } from './ErrorBoundary';
import { App } from './App';
import { registerLeaveGuard } from './services/download-guard.service';
import { registerGlobalShortcuts } from './services/shortcuts.service';

registerLeaveGuard();
registerGlobalShortcuts();

createRoot(document.getElementById('root')!).render(
    <Providers>
        <ErrorBoundary>
            <App />
        </ErrorBoundary>
    </Providers>,
);
