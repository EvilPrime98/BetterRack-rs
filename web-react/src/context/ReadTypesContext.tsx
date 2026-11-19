import { useState, useMemo, type ReactNode } from 'react';
import type { TReadTypes } from '../library.types';
import { ReadTypesContext } from './ReadTypesContext.hooks';

export function ReadTypesProvider({ children }: { children: ReactNode }) {
    const [type, setType] = useState<TReadTypes>('all');

    const value = useMemo(() => ({ type, set: setType }), [type]);

    return <ReadTypesContext.Provider value={value}>{children}</ReadTypesContext.Provider>;
}
