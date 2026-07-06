import { create } from 'zustand';
import type {
  Alert,
  BookTop,
  ConnectionState,
  EngineMode,
  Opportunity,
  PnlPoint,
  Position,
  RiskLimit,
  SystemMetrics,
  Trade,
} from '@/types/api';

const OPP_CAP = 300;
const TRADE_CAP = 500;
const PNL_CAP = 3000;
const ALERT_CAP = 100;

export interface EngineStore {
  mode: EngineMode;
  connection: ConnectionState;
  emergencyStopped: boolean;
  tops: Record<string, BookTop>;
  opportunities: Opportunity[];
  positions: Position[];
  trades: Trade[];
  metrics: SystemMetrics | null;
  alerts: Alert[];
  risk: RiskLimit[];
  pnl: PnlPoint[];

  setMode: (mode: EngineMode) => void;
  setConnection: (c: ConnectionState) => void;
  setEmergency: (v: boolean) => void;
  updateTop: (top: BookTop) => void;
  upsertOpportunity: (o: Opportunity) => void;
  setPositions: (p: Position[]) => void;
  addTrade: (t: Trade) => void;
  setMetrics: (m: SystemMetrics) => void;
  addAlert: (a: Alert) => void;
  dismissAlert: (id: string) => void;
  setRisk: (r: RiskLimit[]) => void;
  pushPnl: (p: PnlPoint) => void;
}

export const useStore = create<EngineStore>((set) => ({
  mode: 'dry_run',
  connection: 'connecting',
  emergencyStopped: false,
  tops: {},
  opportunities: [],
  positions: [],
  trades: [],
  metrics: null,
  alerts: [],
  risk: [],
  pnl: [],

  setMode: (mode) => set({ mode }),
  setConnection: (connection) => set({ connection }),
  setEmergency: (emergencyStopped) => set({ emergencyStopped }),
  updateTop: (top) => set((s) => ({ tops: { ...s.tops, [top.symbol]: top } })),
  upsertOpportunity: (o) =>
    set((s) => {
      const idx = s.opportunities.findIndex((x) => x.id === o.id);
      let next: Opportunity[];
      if (idx >= 0) {
        next = s.opportunities.slice();
        next[idx] = o;
      } else {
        next = [o, ...s.opportunities];
        if (next.length > OPP_CAP) next.length = OPP_CAP;
      }
      return { opportunities: next };
    }),
  setPositions: (positions) => set({ positions }),
  addTrade: (t) =>
    set((s) => {
      const trades = [t, ...s.trades];
      if (trades.length > TRADE_CAP) trades.length = TRADE_CAP;
      return { trades };
    }),
  setMetrics: (metrics) => set({ metrics }),
  addAlert: (a) =>
    set((s) => {
      const alerts = [a, ...s.alerts];
      if (alerts.length > ALERT_CAP) alerts.length = ALERT_CAP;
      return { alerts };
    }),
  dismissAlert: (id) => set((s) => ({ alerts: s.alerts.filter((a) => a.id !== id) })),
  setRisk: (risk) => set({ risk }),
  pushPnl: (p) =>
    set((s) => {
      const pnl = [...s.pnl, p];
      if (pnl.length > PNL_CAP) pnl.shift();
      return { pnl };
    }),
}));
