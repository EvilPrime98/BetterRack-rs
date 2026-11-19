import { UltraComponent } from 'ultra-light-js';
import styles from './item-counter.module.css';
import type { ILibraryResponseItem } from '@/library.types';

export function ItemCounter({
    items,
    subsItems,
}: {
    items: () => ILibraryResponseItem[];
    subsItems: (fn: (value: ILibraryResponseItem[]) => void) => () => void;
}) {
    const onComicsChange = ($span: HTMLElement) => {
        $span.textContent = `${items().length} comics`;
    };

    return UltraComponent({
        component: '<span></span>',
        className: [styles.counter],
        onMount: [onComicsChange],
        trigger: [
            {
                subscriber: subsItems,
                triggerFunction: onComicsChange,
            },
        ],
    });
}
