import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import type { IStoreLink } from '@/store.types';

export interface ILinkPickerModalCtx {
    isVisible: IUltraCompStateStateful<boolean>;
    comicTitle: IUltraCompStateStateful<string>;
    links: IUltraCompStateStateful<IStoreLink[]>;
    openLinkPickerModal: (links: IStoreLink[], comicTitle: string) => Promise<IStoreLink | null>;
    pickLink: (link: IStoreLink) => void;
    closeLinkPickerModal: () => void;
}

let pendingResolve: ((link: IStoreLink | null) => void) | null = null;

function settle(link: IStoreLink | null) {
    const resolve = pendingResolve;
    pendingResolve = null;
    resolve?.(link);
}

export const LINK_PICKER_MODAL_CTX: ILinkPickerModalCtx = ultraCompState({
    isVisible: false,
    comicTitle: '' as string,
    links: [] as IStoreLink[],

    openLinkPickerModal: (
        comp: ILinkPickerModalCtx,
        links: IStoreLink[],
        comicTitle: string,
    ): Promise<IStoreLink | null> => {
        settle(null);
        comp.links.set(links);
        comp.comicTitle.set(comicTitle);
        comp.isVisible.set(true);
        return new Promise<IStoreLink | null>((resolve) => {
            pendingResolve = resolve;
        });
    },

    pickLink: (comp: ILinkPickerModalCtx, link: IStoreLink) => {
        comp.isVisible.set(false);
        settle(link);
    },

    closeLinkPickerModal: (comp: ILinkPickerModalCtx) => {
        comp.isVisible.set(false);
        settle(null);
    },
});
