import { useState, useCallback, useMemo, useRef, type ReactNode } from 'react';
import { useLibraryStore } from '../stores/library.store';
import { NewFolderModalContext } from './NewFolderModalContext.hooks';

export function NewFolderModalProvider({ children }: { children: ReactNode }) {
    const [isVisible, setIsVisible] = useState(false);
    const [parentFolderUid, setParentFolderUid] = useState<string | undefined>(undefined);
    const parentFolderUidRef = useRef<string | undefined>(undefined);

    const openNewFolderModal = useCallback((uid?: string) => {
        parentFolderUidRef.current = uid;
        setParentFolderUid(uid);
        setIsVisible(true);
    }, []);

    const closeNewFolderModal = useCallback(() => {
        setIsVisible(false);
    }, []);

    const submitNewFolder = useCallback(
        async (folderName: string) => {
            const trimmed = folderName.trim();
            if (!trimmed) return;
            closeNewFolderModal();
            await useLibraryStore.getState().createFolder(trimmed, parentFolderUidRef.current);
        },
        [closeNewFolderModal],
    );

    const value = useMemo(
        () => ({
            isVisible,
            parentFolderUid,
            openNewFolderModal,
            closeNewFolderModal,
            submitNewFolder,
        }),
        [isVisible, parentFolderUid, openNewFolderModal, closeNewFolderModal, submitNewFolder],
    );

    return (
        <NewFolderModalContext.Provider value={value}>{children}</NewFolderModalContext.Provider>
    );
}
