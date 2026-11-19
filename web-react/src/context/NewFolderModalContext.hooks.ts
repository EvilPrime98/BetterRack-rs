import { createContext, useContext } from 'react';

export interface INewFolderModalContextValue {
    isVisible: boolean;
    parentFolderUid: string | undefined;
    openNewFolderModal: (parentFolderUid?: string) => void;
    closeNewFolderModal: () => void;
    submitNewFolder: (folderName: string) => Promise<void>;
}

export const NewFolderModalContext = createContext<INewFolderModalContextValue | null>(null);

export function useNewFolderModalContext(): INewFolderModalContextValue {
    const ctx = useContext(NewFolderModalContext);
    if (!ctx)
        throw new Error('useNewFolderModalContext must be used within NewFolderModalProvider');
    return ctx;
}
