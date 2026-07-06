import { createBrowserRouter } from 'react-router-dom';
import { AppShell } from '@/components/layout/AppShell';
import { Backtest } from '@/pages/Backtest';
import { Dashboard } from '@/pages/Dashboard';
import { Opportunities } from '@/pages/Opportunities';
import { Risk } from '@/pages/Risk';
import { Settings } from '@/pages/Settings';
import { Trades } from '@/pages/Trades';

export const router = createBrowserRouter([
  {
    path: '/',
    element: <AppShell />,
    children: [
      { index: true, element: <Dashboard /> },
      { path: 'opportunities', element: <Opportunities /> },
      { path: 'trades', element: <Trades /> },
      { path: 'risk', element: <Risk /> },
      { path: 'backtest', element: <Backtest /> },
      { path: 'settings', element: <Settings /> },
    ],
  },
]);
