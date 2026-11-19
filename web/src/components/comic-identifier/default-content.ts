import { UltraActivity, UltraComponent } from 'ultra-light-js';
import styles from './comic-identifier.module.css';
import { COMIC_IDENT_CTX } from '../../context/identifer-modal.context';
import { Loader } from '../loader/loader';

export function IdentifierDefaultContent({
    isSearching,
    subsIsSearching,
    search,
}: {
    isSearching: () => boolean;
    subsIsSearching: (fn: (value: boolean) => void) => () => void;
    search: () => string;
}) {
    const onTextChange = ($p: HTMLElement) => {
        $p.textContent = search().trim()
            ? 'No comics found'
            : `Start typing to search the wiki for item: ${COMIC_IDENT_CTX.itemUid.get()}`;
    };

    return UltraComponent({
        component: '<div></div>',
        children: [
            Loader({
                mode: {
                    state: isSearching,
                    subscriber: subsIsSearching,
                },
                label: 'Searching…',
            }),

            UltraActivity({
                mode: {
                    state: () => !isSearching(),
                    subscriber: subsIsSearching,
                },
                onMount: [onTextChange],
                component: '<p></p>',
                className: [styles.emptyState],
                trigger: [
                    {
                        subscriber: [COMIC_IDENT_CTX.itemUid.subscribe, subsIsSearching],
                        triggerFunction: onTextChange,
                    },
                ],
            }),
        ],
    });
}
