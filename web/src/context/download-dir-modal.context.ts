import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';

export interface IDownloadDirModalCtx {
    isVisible: IUltraCompStateStateful<boolean>;
    itemTitle: IUltraCompStateStateful<string>;
    heading: IUltraCompStateStateful<string>;
    openDownloadDirModal: (itemTitle: string, heading?: string) => Promise<string | null>;
    confirmDownloadDir: (dir: string) => void;
    closeDownloadDirModal: () => void;
}

const DEFAULT_HEADING = 'Download to:';

let pendingResolve: ((dir: string | null) => void) | null = null;

function settle(dir: string | null) {
    const resolve = pendingResolve;
    pendingResolve = null;
    resolve?.(dir);
}

export const DOWNLOAD_DIR_MODAL_CTX: IDownloadDirModalCtx = ultraCompState({
    isVisible: false,
    itemTitle: '' as string,
    heading: DEFAULT_HEADING as string,

    openDownloadDirModal: (
        comp: IDownloadDirModalCtx,
        itemTitle: string,
        heading: string = DEFAULT_HEADING,
    ): Promise<string | null> => {
        settle(null);
        comp.itemTitle.set(itemTitle);
        comp.heading.set(heading);
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
