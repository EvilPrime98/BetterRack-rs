import { BRDropdown, type BRDropdownOption } from '@/components/br-dropdown/br-dropdown';
import { useReadTypesContext } from '@/context/ReadTypesContext.hooks';
import type { TReadTypes } from '@/library.types';

const OPTIONS: BRDropdownOption<TReadTypes>[] = [
    { value: 'all', label: 'All' },
    { value: 'read', label: 'Read' },
    { value: 'unread', label: 'Unread' },
    { value: 'reading', label: 'Reading' },
];

export function StateFilter() {
    const { type, set } = useReadTypesContext();

    return (
        <BRDropdown<TReadTypes>
            options={OPTIONS}
            value={type}
            onChange={set}
            alignRight
            fit
            aria-label="Read filter"
        />
    );
}
