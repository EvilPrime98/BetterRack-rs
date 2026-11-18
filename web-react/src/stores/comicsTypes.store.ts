import { create } from 'zustand';
import type { TComicsTypes } from '../library.types';
import { useUserPrefStore } from './userPref.store';

interface IComicsTypeStore {
    type: TComicsTypes;
    init: () => void;
    set: (type: TComicsTypes) => void;
}

export const useComicsTypeStore = create<IComicsTypeStore>((set) => ({
    type: 'detail',

    init: () => {
        const comicType = useUserPrefStore.getState().getPref('comicType');
        if (comicType) set({ type: comicType });
    },

    set: (type) => {
        set({ type });
        useUserPrefStore.getState().setPref({ comicType: type });
    },
}));
