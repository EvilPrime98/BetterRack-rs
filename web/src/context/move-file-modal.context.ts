import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import { LIBRARY_CONTEXT } from './library.context';

export interface IMoveFileModalCtx {
    isVisible: IUltraCompStateStateful<boolean>;
    fileUid: IUltraCompStateStateful<string>;
    fileName: IUltraCompStateStateful<string>;
    openMoveFileModal: (fileUid: string, fileName: string) => void;
    closeMoveFileModal: () => void;
    selectMoveTarget: (targetFolderUid?: string) => Promise<void>;
    pickFolder: () => Promise<{ folder?: string } | null>;
}

let pendingPick: ((choice: { folder?: string } | null) => void) | null = null;

function settlePick(choice: { folder?: string } | null) {
    const resolve = pendingPick;
    pendingPick = null;
    resolve?.(choice);
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
        settlePick(null);
    },

    pickFolder: (comp: IMoveFileModalCtx) => {
        settlePick(null);
        comp.fileUid.set('');
        comp.fileName.set('');
        comp.isVisible.set(true);
        return new Promise<{ folder?: string } | null>((resolve) => {
            pendingPick = resolve;
        });
    },

    selectMoveTarget: async (comp: IMoveFileModalCtx, targetFolderUid?: string) => {
        if (pendingPick) {
            comp.isVisible.set(false);
            settlePick({ folder: targetFolderUid });
            return;
        }
        const fileUid = comp.fileUid.get();
        comp.closeMoveFileModal();
        await LIBRARY_CONTEXT.moveFile(fileUid, targetFolderUid);
    },
});
