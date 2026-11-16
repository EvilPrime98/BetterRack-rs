import styles from '../../pages/reader.page.module.css';

export function ReaderPageProgressBar({
    currentPage,
    totalPages,
}: {
    currentPage: number;
    totalPages: number;
}) {
    const total = totalPages || 1;
    const per = (currentPage / total) * 100;

    return (
        <div className={styles.progressTrack}>
            <div className={styles.progressFill} style={{ width: `${per}%` }} />
        </div>
    );
}
