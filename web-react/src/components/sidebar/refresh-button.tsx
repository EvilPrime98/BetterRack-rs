import { useState } from 'react';
import { useLibraryStore } from '@/stores/library.store';
import styles from './sidebar.module.css';

export function RefreshLibraryButton() {
    // useLibraryStore doesn't expose a shared fetching flag, so "spinning" reflects only
    // this button's own refreshLibrary() call, not any in-flight fetch from elsewhere.
    const [isRefreshing, setIsRefreshing] = useState(false);
    const refreshLibrary = useLibraryStore((s) => s.refreshLibraryWithPrompt);
    const identifyProgress = useLibraryStore((s) => s.identifyProgress);
    const isBusy = isRefreshing || identifyProgress !== null;

    async function handleRefresh() {
        setIsRefreshing(true);
        try {
            await refreshLibrary();
        } finally {
            setIsRefreshing(false);
        }
    }

    return (
        <button
            className={[styles.refreshButton, isBusy ? styles.spinning : '']
                .filter(Boolean)
                .join(' ')}
            aria-label="Refresh library"
            onClick={handleRefresh}
            disabled={isBusy}
        >
            <span className={styles.refreshSpinner}></span>
            <span>
                {identifyProgress
                    ? `Identifying${identifyProgress.total ? ` ${identifyProgress.done}/${identifyProgress.total}` : '…'}`
                    : 'Refresh Libraries'}
            </span>
        </button>
    );
}
