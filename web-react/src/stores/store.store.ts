import { create } from 'zustand';
import type { IStorePost } from '../store.types';

interface IStorePageState {
    query: string;
    activeSearch: string;
    results: IStorePost[];
    page: number;
    hasMore: boolean;
    hasLoaded: boolean;
    scrollY: number;
    set: (patch: Partial<Omit<IStorePageState, 'set'>>) => void;
}

export const useStorePageStore = create<IStorePageState>((set) => ({
    query: '',
    activeSearch: '',
    results: [],
    page: 1,
    hasMore: true,
    hasLoaded: false,
    scrollY: 0,

    set: (patch) => set(patch),
}));
