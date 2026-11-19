import styles from './read-bard.module.css';

export function ReadBar({
    readPercentage,
}: {
    readPercentage: number; // 0-100
}) {
    return (
        <div className={styles.readBar}>
            <div
                className={styles.fillIn}
                style={{
                    width: `${readPercentage}%`,
                    backgroundColor: readPercentage === 100 ? '#13a629' : '#34c3d1',
                }}
            />
        </div>
    );
}
