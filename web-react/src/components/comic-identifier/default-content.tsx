import styles from './comic-identifier.module.css';
import { useComicIdentStore } from '@/stores/comicIdent.store';
import { Loader } from '../loader/loader';

export function IdentifierDefaultContent({
    isSearching,
    search,
}: {
    isSearching: boolean;
    search: string;
}) {
    const itemUid = useComicIdentStore((s) => s.itemUid);

    const message = search.trim()
        ? 'No comics found'
        : `Start typing to search the wiki for item: ${itemUid}`;

    return (
        <div>
            <Loader visible={isSearching} label="Searching…" />

            <p className={styles.emptyState} style={{ display: !isSearching ? undefined : 'none' }}>
                {message}
            </p>
        </div>
    );
}
