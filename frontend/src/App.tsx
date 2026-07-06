import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider } from 'react-router-dom';
import { ControlProvider } from '@/data/ControlContext';
import { router } from './router';

const queryClient = new QueryClient({
  defaultOptions: { queries: { staleTime: 5_000, refetchOnWindowFocus: false } },
});

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ControlProvider>
        <RouterProvider router={router} />
      </ControlProvider>
    </QueryClientProvider>
  );
}
