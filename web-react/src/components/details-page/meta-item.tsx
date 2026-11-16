import styles from '@/pages/details-page.module.css';

export function MetaItem({ label, value }: { label: string; value: string }) {
    return (
        <div className={styles.metaItem}>
            <span className={styles.metaLabel}>{label}</span>
            <span className={styles.metaValue}>{value}</span>
        </div>
    );
}
