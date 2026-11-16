import { useState, useCallback, useMemo, useRef, type ReactNode } from 'react';
import type { IStoreLink } from '@/store.types';
import { LinkPickerModalContext } from './LinkPickerModalContext.hooks';

export function LinkPickerModalProvider({ children }: { children: ReactNode }) {
    const [isVisible, setIsVisible] = useState(false);
    const [comicTitle, setComicTitle] = useState('');
    const [links, setLinks] = useState<IStoreLink[]>([]);
    const pendingResolve = useRef<((link: IStoreLink | null) => void) | null>(null);

    const settle = useCallback((link: IStoreLink | null) => {
        const resolve = pendingResolve.current;
        pendingResolve.current = null;
        resolve?.(link);
    }, []);

    const openLinkPickerModal = useCallback(
        (nextLinks: IStoreLink[], title: string) => {
            settle(null);
            setLinks(nextLinks);
            setComicTitle(title);
            setIsVisible(true);
            return new Promise<IStoreLink | null>((resolve) => {
                pendingResolve.current = resolve;
            });
        },
        [settle],
    );

    const pickLink = useCallback(
        (link: IStoreLink) => {
            setIsVisible(false);
            settle(link);
        },
        [settle],
    );

    const closeLinkPickerModal = useCallback(() => {
        setIsVisible(false);
        settle(null);
    }, [settle]);

    const value = useMemo(
        () => ({
            isVisible,
            comicTitle,
            links,
            openLinkPickerModal,
            pickLink,
            closeLinkPickerModal,
        }),
        [isVisible, comicTitle, links, openLinkPickerModal, pickLink, closeLinkPickerModal],
    );

    return (
        <LinkPickerModalContext.Provider value={value}>{children}</LinkPickerModalContext.Provider>
    );
}
