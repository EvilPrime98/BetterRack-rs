import { create } from 'zustand';
import type { TComicsTypes } from '../library.types';
import { useUserPrefStore } from './userPref.store';

interface IComicsTypeStore {
    type: TComicsTypes;
    init: () => void;
    next: () => void;
}

export const useComicsTypeStore = create<IComicsTypeStore>((set, get) => ({
    type: 'detail',

    init: () => {
        const comicType = useUserPrefStore.getState().getPref('comicType');
        if (comicType) set({ type: comicType });
    },

    next: () => {
        const nextType = get().type === 'cover' ? 'detail' : 'cover';
        set({ type: nextType });
        useUserPrefStore.getState().setPref({ comicType: nextType });
    },
}));
