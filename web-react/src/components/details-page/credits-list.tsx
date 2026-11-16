import styles from '@/pages/details-page.module.css';
import { COMIC_FILTERS } from '@/library.types';
import type { CreditRow, Navigate } from './details-page.types';

export function CreditsList({
    creditRows,
    firstWriter,
    navigate,
}: {
    creditRows: CreditRow[];
    firstWriter?: string;
    navigate: Navigate;
}) {
    if (creditRows.length === 0) return null;
    return (
        <div className={styles.credits}>
            {creditRows.map((row) => {
                const clickable = row.label === 'Writer' && Boolean(firstWriter);
                const rowContent = (
                    <>
                        <span className={styles.creditLabel}>{row.label}</span>
                        <span className={styles.creditValue}>{row.names.join(', ')}</span>
                    </>
                );
                return clickable ? (
                    <button
                        key={row.label}
                        type="button"
                        className={[styles.creditRow, styles.creditRowClickable].join(' ')}
                        onClick={() =>
                            navigate(
                                `/filters?${COMIC_FILTERS.writer}=${encodeURIComponent(firstWriter as string)}`,
                            )
                        }
                    >
                        {rowContent}
                    </button>
                ) : (
                    <div key={row.label} className={styles.creditRow}>
                        {rowContent}
                    </div>
                );
            })}
        </div>
    );
}
