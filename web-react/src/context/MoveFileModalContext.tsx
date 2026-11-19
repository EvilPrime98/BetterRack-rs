import { useState, useCallback, useMemo, useRef, type ReactNode } from 'react';
import { useLibraryStore } from '../stores/library.store';
import { MoveFileModalContext, type IFolderChoice } from './MoveFileModalContext.hooks';

export function MoveFileModalProvider({ children }: { children: ReactNode }) {
    const [isVisible, setIsVisible] = useState(false);
    const [fileUid, setFileUid] = useState('');
    const [fileName, setFileName] = useState('');

    const pendingPick = useRef<((choice: IFolderChoice | null) => void) | null>(null);

    const settlePick = useCallback((choice: IFolderChoice | null) => {
        const resolve = pendingPick.current;
        pendingPick.current = null;
        resolve?.(choice);
    }, []);

    const openMoveFileModal = useCallback((openFileUid: string, openFileName: string) => {
        setFileUid(openFileUid);
        setFileName(openFileName);
        setIsVisible(true);
    }, []);

    const closeMoveFileModal = useCallback(() => {
        setIsVisible(false);
        settlePick(null);
    }, [settlePick]);

    const pickFolder = useCallback(() => {
        settlePick(null);
        setFileUid('');
        setFileName('');
        setIsVisible(true);
        return new Promise<IFolderChoice | null>((resolve) => {
            pendingPick.current = resolve;
        });
    }, [settlePick]);

    const selectMoveTarget = useCallback(
        async (targetFolderUid?: string) => {
            if (pendingPick.current) {
                setIsVisible(false);
                settlePick({ folder: targetFolderUid });
                return;
            }
            closeMoveFileModal();
            await useLibraryStore.getState().moveFile(fileUid, targetFolderUid);
        },
        [fileUid, closeMoveFileModal, settlePick],
    );

    const value = useMemo(
        () => ({
            isVisible,
            fileUid,
            fileName,
            openMoveFileModal,
            closeMoveFileModal,
            selectMoveTarget,
            pickFolder,
        }),
        [
            isVisible,
            fileUid,
            fileName,
            openMoveFileModal,
            closeMoveFileModal,
            selectMoveTarget,
            pickFolder,
        ],
    );

    return <MoveFileModalContext.Provider value={value}>{children}</MoveFileModalContext.Provider>;
}
