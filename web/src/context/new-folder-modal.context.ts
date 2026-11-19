import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import { LIBRARY_CONTEXT } from './library.context';

export interface INewFolderModalCtx {
    isVisible: IUltraCompStateStateful<boolean>;
    parentFolderUid: IUltraCompStateStateful<string | null>;
    openNewFolderModal: (parentFolderUid?: string) => void;
    closeNewFolderModal: () => void;
    submitNewFolder: (folderName: string) => Promise<void>;
}

export const NEW_FOLDER_MODAL_CTX: INewFolderModalCtx = ultraCompState({
    isVisible: false,

    parentFolderUid: null as string | null,

    openNewFolderModal: (comp: INewFolderModalCtx, parentFolderUid?: string) => {
        comp.parentFolderUid.set(parentFolderUid ?? null);
        comp.isVisible.set(true);
    },

    closeNewFolderModal: (comp: INewFolderModalCtx) => {
        comp.isVisible.set(false);
    },

    submitNewFolder: async (comp: INewFolderModalCtx, folderName: string) => {
        const trimmed = folderName.trim();
        if (!trimmed) return;
        comp.closeNewFolderModal();
        await LIBRARY_CONTEXT.createFolder(
            trimmed,
            NEW_FOLDER_MODAL_CTX.parentFolderUid.get() ?? undefined,
        );
    },
});
