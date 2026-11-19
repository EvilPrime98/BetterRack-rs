import { ultraCompState, type IUltraCompStateStateful } from 'ultra-light-js';
import { APP_NAME } from '../data';

export interface IDocumentTitleCtx {
    title: IUltraCompStateStateful<string>;
    setTitle: (title?: string) => void;
    reset: () => void;
}

export const DOCUMENT_TITLE_CONTEXT: IDocumentTitleCtx = ultraCompState({
    title: APP_NAME,

    setTitle: (comp: IDocumentTitleCtx, title?: string) => {
        const fullTitle = title ? `${title} · ${APP_NAME}` : APP_NAME;
        comp.title.set(fullTitle);
        document.title = fullTitle;
    },

    reset: (comp: IDocumentTitleCtx) => {
        comp.title.set(APP_NAME);
        document.title = APP_NAME;
    },
});
