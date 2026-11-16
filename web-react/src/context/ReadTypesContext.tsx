import { useState, useCallback, useMemo, type ReactNode } from 'react';
import type { TReadTypes } from '../library.types';
import { ReadTypesContext } from './ReadTypesContext.hooks';

export function ReadTypesProvider({ children }: { children: ReactNode }) {
    const [type, setType] = useState<TReadTypes>('all');

    const next = useCallback(() => {
        setType((currType) => {
            if (currType === 'all') return 'read';
            if (currType === 'read') return 'reading';
            if (currType === 'reading') return 'unread';
            return 'all';
        });
    }, []);

    const value = useMemo(() => ({ type, next }), [type, next]);

    return <ReadTypesContext.Provider value={value}>{children}</ReadTypesContext.Provider>;
}
