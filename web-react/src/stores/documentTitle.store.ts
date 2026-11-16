import { create } from 'zustand';
import { APP_NAME } from '../data';

interface IDocumentTitleStore {
    title: string;
    setTitle: (title?: string) => void;
    reset: () => void;
}

export const useDocumentTitleStore = create<IDocumentTitleStore>((set) => ({
    title: APP_NAME,

    setTitle: (title) => {
        const fullTitle = title ? `${title} · ${APP_NAME}` : APP_NAME;
        set({ title: fullTitle });
        document.title = fullTitle;
    },

    reset: () => {
        set({ title: APP_NAME });
        document.title = APP_NAME;
    },
}));
