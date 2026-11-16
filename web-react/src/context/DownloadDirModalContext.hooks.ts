import { createContext, useContext } from 'react';

export interface IDownloadDirModalContextValue {
    isVisible: boolean;
    itemTitle: string;
    heading: string;
    openDownloadDirModal: (itemTitle: string, heading?: string) => Promise<string | null>;
    confirmDownloadDir: (dir: string) => void;
    closeDownloadDirModal: () => void;
}

export const DownloadDirModalContext = createContext<IDownloadDirModalContextValue | null>(null);

export function useDownloadDirModalContext(): IDownloadDirModalContextValue {
    const ctx = useContext(DownloadDirModalContext);
    if (!ctx)
        throw new Error('useDownloadDirModalContext must be used within DownloadDirModalProvider');
    return ctx;
}
