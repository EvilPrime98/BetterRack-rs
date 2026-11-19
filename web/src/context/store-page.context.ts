import { ultraCompState } from 'ultra-light-js';
import type { IStorePost } from '../store.types';

export const STORE_PAGE_CONTEXT = ultraCompState({
    query: '',
    activeSearch: '',
    results: [] as IStorePost[],
    page: 1,
    hasMore: true,
    hasLoaded: false,
    scrollY: 0,
});
