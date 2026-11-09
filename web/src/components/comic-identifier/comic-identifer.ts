import { UltraActivity, UltraComponent, ultraState } from 'ultra-light-js';
import styles from './comic-identifier.module.css';
import type { WikiComic } from 'better-wiki';
import { SearchIcon } from '../../icons/search.icon';
import { CloseIcon } from '../../icons/close.icon';
import { SuggestionCard } from './suggestion-card';
import { IdentifierDefaultContent } from './default-content';
import { COMIC_IDENT_CTX } from '../../context/identifer-modal.context';
import { fetchComics } from '../../services/wiki.service';
import { CrButton } from '../cr-button/cr-button';
import { LIBRARY_CONTEXT } from '../../context/library.context';
import { CONFIRM_MODAL_CTX } from '../../context/confirm-modal.context';

export function ComicIdentifier() {
    const DEBOUNCING_DELAY = 500;
    let timeOutID: number | null = null;
    const [search, setSearch, subsSearch] = ultraState('');
    const [suggestions, setSuggestions, subsSuggestions] = ultraState<WikiComic[]>([]);
    const [isSearching, setIsSearching, subsIsSearching] = ultraState(false);

    const close = () => COMIC_IDENT_CTX.isVisible.set(false);

    const onUnidentify = async () => {
        const confirmed = await CONFIRM_MODAL_CTX.confirmDialog({
            title: 'Un-identify comic?',
            message: 'This will remove the identified metadata for this comic.',
            confirmLabel: 'Un-identify',
        });
        if (!confirmed) return;
        await LIBRARY_CONTEXT.unidentifyFile(COMIC_IDENT_CTX.itemUid.get());
        close();
    };

    const getSuggestions = async (search: string) => {
        const comics = await fetchComics(search, 120);
        setSuggestions(comics);
        setIsSearching(false);
    };

    const onInput = (e: Event) => {
        const $input = e.currentTarget as HTMLInputElement;
        setSearch($input.value);
    };

    const onSuggestionsChange = ($section: HTMLElement) => {
        const items = suggestions();
        $section.replaceChildren(
            ...(items.length
                ? items.map((s) => SuggestionCard({ comic: s }))
                : [IdentifierDefaultContent({ search, isSearching, subsIsSearching })]),
        );
    };

    const onKeydown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') close();
    };

    subsSearch((value: string) => {
        if (timeOutID) {
            clearTimeout(timeOutID);
        }

        if (!value.trim()) {
            setSuggestions([]);
            setIsSearching(false);
            return;
        }

        timeOutID = setTimeout(() => {
            setIsSearching(true);
            getSuggestions(value);
        }, DEBOUNCING_DELAY);
    });

    return UltraActivity({
        mode: {
            state: COMIC_IDENT_CTX.isVisible.get,
            subscriber: COMIC_IDENT_CTX.isVisible.subscribe,
        },

        component: '<div></div>',

        className: [styles.overlay],

        onMount: [
            () => {
                document.addEventListener('keydown', onKeydown);
                return () => document.removeEventListener('keydown', onKeydown);
            },
        ],

        children: [
            UltraComponent({
                component: '<div></div>',
                className: [styles.backdrop],
                eventHandler: { click: close },
            }),

            UltraComponent({
                component: '<div></div>',

                attributes: {
                    role: 'dialog',
                    'aria-modal': 'true',
                    'aria-label': 'Search comics',
                },

                className: [styles.modal],

                children: [
                    UltraComponent({
                        component: '<div></div>',

                        className: [styles.searchBar],

                        children: [
                            SearchIcon({ size: 16, color: '#8f8f8f' }),

                            UltraComponent({
                                component: '<input/>',
                                attributes: {
                                    type: 'text',
                                    placeholder: 'Search a comic..',
                                },
                                eventHandler: {
                                    input: onInput,
                                },
                                onMount: [($input) => ($input as HTMLInputElement).focus()],
                            }),

                            CrButton({
                                text: 'Unidentify',
                                variant: 'red',
                                className: [styles.unidentifyButton],
                                attributes: {
                                    'aria-label': 'Un-identify this comic',
                                },
                                eventHandler: {
                                    click: onUnidentify,
                                },
                            }),

                            UltraComponent({
                                component: CloseIcon({ size: 14 }),
                                className: [styles.closeButton],
                                attributes: { role: 'button', 'aria-label': 'Close search' },
                                eventHandler: { click: close },
                            }),
                        ],
                    }),

                    UltraComponent({
                        component: '<section></section>',
                        className: [styles.suggestions],
                        onMount: [onSuggestionsChange],
                        trigger: [
                            {
                                subscriber: [subsSuggestions, subsIsSearching],
                                triggerFunction: onSuggestionsChange,
                            },
                        ],
                    }),
                ],
            }),
        ],
    });
}
