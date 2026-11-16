import { createContext, useContext } from 'react';
import type { IStoreLink } from '@/store.types';

export interface ILinkPickerModalContextValue {
    isVisible: boolean;
    comicTitle: string;
    links: IStoreLink[];
    openLinkPickerModal: (links: IStoreLink[], comicTitle: string) => Promise<IStoreLink | null>;
    pickLink: (link: IStoreLink) => void;
    closeLinkPickerModal: () => void;
}

export const LinkPickerModalContext = createContext<ILinkPickerModalContextValue | null>(null);

export function useLinkPickerModalContext(): ILinkPickerModalContextValue {
    const ctx = useContext(LinkPickerModalContext);
    if (!ctx)
        throw new Error('useLinkPickerModalContext must be used within LinkPickerModalProvider');
    return ctx;
}
