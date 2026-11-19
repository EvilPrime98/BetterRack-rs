import { UltraComponent, ultraNavigate, ultraQueryParams } from 'ultra-light-js';
import styles from './sidebar.module.css';
import { LIBRARY_CONTEXT } from '../../context/library.context';
import { SearchIcon } from '../../icons/search.icon';
import { SIDEBAR_CONTEXT } from '../../context/sidebar.context';

export function SidebarSearch() {
    function leaveSearchResults() {
        if (ultraQueryParams().search) ultraNavigate({ href: '/' });
    }

    function clearSearch($input: HTMLInputElement) {
        $input.value = '';
        $input.blur();
        LIBRARY_CONTEXT.searchQuery.set('');
        leaveSearchResults();
    }

    function onSearchMount($el: HTMLElement) {
        const $input = $el as HTMLInputElement;
        if (!$input) return;
        $input.value = LIBRARY_CONTEXT.searchQuery.get();
        if (ultraQueryParams().search) {
            const end = $input.value.length;
            $input.setSelectionRange(end, end);
        }
    }

    function onSearchKeydown(e: Event) {
        const key = (e as KeyboardEvent).key;

        if (key === 'Escape') {
            clearSearch(e.target as HTMLInputElement);
            return;
        }

        if (key !== 'Enter') return;

        const query = (e.target as HTMLInputElement).value.trim();
        const onSearchPage = !!ultraQueryParams().search;

        LIBRARY_CONTEXT.searchQuery.set(query);
        SIDEBAR_CONTEXT.isExpanded.set(false);

        if (query) {
            ultraNavigate({ href: `/?search=${encodeURIComponent(query)}` });
        } else if (onSearchPage) {
            ultraNavigate({ href: '/' });
        }
    }

    return UltraComponent({
        component: '<div></div>',

        className: [styles.searchBox],

        children: [
            UltraComponent({
                component: SearchIcon({ size: 16, color: '#8a8a8a' }),
                className: [styles.searchIcon],
            }),

            UltraComponent({
                onMount: [onSearchMount],
                component: '<input type="text" />',
                className: [styles.searchInput],
                attributes: {
                    placeholder: 'Search library…',
                    'aria-label': 'Search library',
                },
                eventHandler: {
                    keydown: onSearchKeydown,
                },
            }),
        ],
    });
}
