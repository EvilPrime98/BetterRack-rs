import { createContext, useContext } from 'react';

export interface IFolderChoice {
    folder?: string;
}

export interface IMoveFileModalContextValue {
    isVisible: boolean;
    fileUid: string;
    fileName: string;
    openMoveFileModal: (fileUid: string, fileName: string) => void;
    closeMoveFileModal: () => void;
    selectMoveTarget: (targetFolderUid?: string) => Promise<void>;
    pickFolder: () => Promise<IFolderChoice | null>;
}

export const MoveFileModalContext = createContext<IMoveFileModalContextValue | null>(null);

export function useMoveFileModalContext(): IMoveFileModalContextValue {
    const ctx = useContext(MoveFileModalContext);
    if (!ctx) throw new Error('useMoveFileModalContext must be used within MoveFileModalProvider');
    return ctx;
}
