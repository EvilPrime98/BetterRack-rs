import { useSearchParams } from 'react-router-dom';
import { COMIC_FILTERS, type IComicFilters } from '../library.types';

export function useComicFilters(): IComicFilters {
    const [searchParams] = useSearchParams();
    return Object.fromEntries([...searchParams.entries()].filter(([key]) => key in COMIC_FILTERS));
}
