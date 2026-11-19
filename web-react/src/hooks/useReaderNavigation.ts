import { useCallback, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';

export function useReaderNavigation() {
    const navigate = useNavigate();

    const goBack = useCallback(() => {
        if (window.history.length > 1) window.history.back();
        else navigate('/');
    }, [navigate]);

    useEffect(() => {
        function onKeydown(e: KeyboardEvent) {
            if (e.key === 'Escape') goBack();
        }
        window.addEventListener('keydown', onKeydown);
        return () => window.removeEventListener('keydown', onKeydown);
    }, [goBack]);

    return { navigate, goBack };
}
