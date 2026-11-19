import styles from './item-counter.module.css';

export function ItemCounter({
    count,
    label = 'comics',
    className,
}: {
    count: number;
    label?: string;
    className?: string;
}) {
    return (
        <span className={[styles.counter, className].filter(Boolean).join(' ')}>
            {count} {label}
        </span>
    );
}
