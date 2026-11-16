import { useEffect, useRef, type InputHTMLAttributes } from 'react';
import type { TFieldKey } from '@/settings.types';
import styles from '@/pages/settings.page.module.css';
import { useSettingsStore } from '@/stores/settings.store';

interface TextFieldProps extends InputHTMLAttributes<HTMLInputElement> {
    /** Source used a prop named `key`, which is a reserved React prop name and can't be read from props — renamed. */
    fieldKey: TFieldKey;
    label: string;
}

export function TextField({ fieldKey, label, placeholder, className, ...props }: TextFieldProps) {
    const inputRef = useRef<HTMLInputElement>(null);
    const settings = useSettingsStore((s) => s.settings);

    useEffect(() => {
        if (inputRef.current) inputRef.current.value = settings[fieldKey] ?? '';
    }, [settings, fieldKey]);

    return (
        <div className={styles.field}>
            <label className={styles.label} htmlFor={`settings-${fieldKey}`}>
                {label}
            </label>

            <input
                ref={inputRef}
                id={`settings-${fieldKey}`}
                type="text"
                placeholder={placeholder ?? ''}
                className={[styles.input, className].filter(Boolean).join(' ')}
                {...props}
            />
        </div>
    );
}
