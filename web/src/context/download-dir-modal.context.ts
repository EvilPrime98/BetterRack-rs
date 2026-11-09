import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';

export interface IDownloadDirModalCtx {
    isVisible: IUltraCompStateStateful<boolean>;
    itemTitle: IUltraCompStateStateful<string>;
    openDownloadDirModal: (itemTitle: string) => Promise<string | null>;
    confirmDownloadDir: (dir: string) => void;
    closeDownloadDirModal: () => void;
}

let pendingResolve: ((dir: string | null) => void) | null = null;

function settle(dir: string | null) {
    const resolve = pendingResolve;
    pendingResolve = null;
    resolve?.(dir);
}

export const DOWNLOAD_DIR_MODAL_CTX: IDownloadDirModalCtx = ultraCompState({
    isVisible: false,
    itemTitle: '' as string,

    openDownloadDirModal: (
        comp: IDownloadDirModalCtx,
        itemTitle: string,
    ): Promise<string | null> => {
        settle(null);
        comp.itemTitle.set(itemTitle);
        comp.isVisible.set(true);
        return new Promise<string | null>((resolve) => {
            pendingResolve = resolve;
        });
    },

    confirmDownloadDir: (comp: IDownloadDirModalCtx, dir: string) => {
        comp.isVisible.set(false);
        settle(dir);
    },

    closeDownloadDirModal: (comp: IDownloadDirModalCtx) => {
        comp.isVisible.set(false);
        settle(null);
    },
});
