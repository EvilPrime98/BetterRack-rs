import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import { LIBRARY_CONTEXT } from './library.context';

export interface IMoveFileModalCtx {
    isVisible: IUltraCompStateStateful<boolean>;
    fileUid: IUltraCompStateStateful<string>;
    fileName: IUltraCompStateStateful<string>;
    openMoveFileModal: (fileUid: string, fileName: string) => void;
    closeMoveFileModal: () => void;
    selectMoveTarget: (targetFolderUid?: string) => Promise<void>;
}

export const MOVE_FILE_MODAL_CTX: IMoveFileModalCtx = ultraCompState({
    isVisible: false,
    fileUid: '' as string,
    fileName: '' as string,

    openMoveFileModal: (comp: IMoveFileModalCtx, fileUid: string, fileName: string) => {
        comp.fileUid.set(fileUid);
        comp.fileName.set(fileName);
        comp.isVisible.set(true);
    },

    closeMoveFileModal: (comp: IMoveFileModalCtx) => {
        comp.isVisible.set(false);
    },

    selectMoveTarget: async (comp: IMoveFileModalCtx, targetFolderUid?: string) => {
        const fileUid = comp.fileUid.get();
        comp.closeMoveFileModal();
        await LIBRARY_CONTEXT.moveFile(fileUid, targetFolderUid);
    },
});
