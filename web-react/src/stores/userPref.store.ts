import { create } from 'zustand';
import type { TComicsTypes, TFilterOptions } from '../library.types';

export type TReadingMode = 'vertical' | 'horizontal';

export interface IUserPref {
    filter: TFilterOptions;
    comicType: TComicsTypes;
    /** Whether or not to show a confirmation modal when stopping a download job. */
    askStopDownloads: boolean;
    zoom: number;
    readingMode: TReadingMode;
}

interface IUserPrefStore {
    keyName: string;
    pref: IUserPref;
    compile: () => void;
    init: () => void;
    getPref: <K extends keyof IUserPref>(prefKey: K) => IUserPref[K];
    setPref: (pref: Partial<IUserPref>) => void;
}

const DEFAULT_PREF: IUserPref = {
    filter: 'Alphabetically',
    comicType: 'detail',
    askStopDownloads: true,
    zoom: 1,
    readingMode: 'vertical',
};

export const useUserPrefStore = create<IUserPrefStore>((set, get) => ({
    keyName: 'better-rack-user-pref',

    pref: DEFAULT_PREF,

    compile: () => {
        window.localStorage.setItem(get().keyName, JSON.stringify(get().pref));
    },

    init: () => {
        const lsCache = window.localStorage.getItem(get().keyName);
        if (!lsCache) {
            get().compile();
        } else {
            set({ pref: { ...DEFAULT_PREF, ...JSON.parse(lsCache) } });
        }
    },

    getPref: (prefKey) => get().pref[prefKey],

    setPref: (pref) => {
        const currPref = structuredClone(get().pref);
        Object.assign(currPref, pref);
        set({ pref: currPref });
        get().compile();
    },
}));
