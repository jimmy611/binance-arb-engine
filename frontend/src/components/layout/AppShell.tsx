import { Outlet } from 'react-router-dom';
import { Sidebar } from './Sidebar';
import { StatusStrip } from './StatusStrip';
import { TopBar } from './TopBar';

export function AppShell() {
  return (
    <div className="flex h-full overflow-hidden">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <TopBar />
        <StatusStrip />
        <main className="min-h-0 flex-1 overflow-auto p-4">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
