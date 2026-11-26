import {useMemo} from 'react';
import {useToast} from '@astryxdesign/core/Toast';

function messageWithDescription(message: string, description?: string) {
  return description ? `${message} — ${description}` : message;
}

export function useAppToast() {
  const show = useToast();
  return useMemo(
    () => ({
      info(message: string, description?: string) {
        show({body: messageWithDescription(message, description)});
      },
      success(message: string, description?: string) {
        show({body: messageWithDescription(message, description)});
      },
      error(message: string, description?: string) {
        show({body: messageWithDescription(message, description), type: 'error'});
      },
    }),
    [show],
  );
}
