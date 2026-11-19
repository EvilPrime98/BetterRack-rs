import { useEffect, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import styles from './sidebar.module.css';
import { useLibraryStore } from '@/stores/library.store';
import { SearchIcon } from '@/icons/search.icon';
import { useSidebarStore } from '@/stores/sidebar.store';

export function SidebarSearch() {
    const navigate = useNavigate();
    const [searchParams] = useSearchParams();
    const inputRef = useRef<HTMLInputElement>(null);

    const searchQuery = useLibraryStore((s) => s.searchQuery);
    const setSearchQuery = useLibraryStore((s) => s.setSearchQuery);
    const setIsExpanded = useSidebarStore((s) => s.setIsExpanded);

    const [hadSearchParamOnMount] = useState(() => !!searchParams.get('search'));

    function leaveSearchResults() {
        if (searchParams.get('search')) navigate('/');
    }

    function clearSearch($input: HTMLInputElement) {
        $input.value = '';
        $input.blur();
        setSearchQuery('');
        leaveSearchResults();
    }

    useEffect(() => {
        const $input = inputRef.current;
        if (!$input) return;
        $input.value = useLibraryStore.getState().searchQuery;
        if (hadSearchParamOnMount) {
            const end = $input.value.length;
            $input.setSelectionRange(end, end);
        }
    }, [hadSearchParamOnMount]);

    function onSearchKeydown(e: React.KeyboardEvent<HTMLInputElement>) {
        const key = e.key;

        if (key === 'Escape') {
            clearSearch(e.target as HTMLInputElement);
            return;
        }

        if (key !== 'Enter') return;

        const query = (e.target as HTMLInputElement).value.trim();
        const onSearchPage = !!searchParams.get('search');

        setSearchQuery(query);
        setIsExpanded(false);

        if (query) {
            navigate(`/?search=${encodeURIComponent(query)}`);
        } else if (onSearchPage) {
            navigate('/');
        }
    }

    return (
        <div className={styles.searchBox}>
            <div className={styles.searchIcon}>
                <SearchIcon size={16} color="#8a8a8a" />
            </div>

            <input
                ref={inputRef}
                type="text"
                className={styles.searchInput}
                placeholder="Search library…"
                aria-label="Search library"
                defaultValue={searchQuery}
                onKeyDown={onSearchKeydown}
            />
        </div>
    );
}
