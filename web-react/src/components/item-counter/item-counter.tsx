import styles from './item-counter.module.css';
import type { ILibraryResponseItem } from '@/library.types';

export function ItemCounter({
    items,
    className,
}: {
    items: ILibraryResponseItem[];
    className?: string;
}) {
    return (
        <span className={[styles.counter, className].filter(Boolean).join(' ')}>
            {items.length} comics
        </span>
    );
}
