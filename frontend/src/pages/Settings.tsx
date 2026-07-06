import { useState } from 'react';
import { Modal } from '@/components/ui/Modal';
import { Badge, Button, Panel } from '@/components/ui/primitives';
import { useControl } from '@/data/ControlContext';
import { useStore } from '@/store/useStore';

const LIVE_PHRASE = 'ENABLE LIVE TRADING';

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block space-y-1">
      <span className="text-2xs uppercase tracking-wider text-muted">{label}</span>
      {children}
    </label>
  );
}

const inputClass = 'focus-ring h-9 w-full rounded-lg border border-border/60 bg-bg px-3 text-sm';

export function Settings() {
  const { control } = useControl();
  const mode = useStore((s) => s.mode);
  const setMode = useStore((s) => s.setMode);

  const [liveOpen, setLiveOpen] = useState(false);
  const [phrase, setPhrase] = useState('');
  const [msg, setMsg] = useState('');

  const [execMode, setExecMode] = useState('parallel_ioc');
  const [cycleLen, setCycleLen] = useState(3);
  const [maxPerTrade, setMaxPerTrade] = useState(500);
  const [maxOpen, setMaxOpen] = useState(2000);
  const [dailyLoss, setDailyLoss] = useState(500);
  const [minEdge, setMinEdge] = useState(5);
  const [allow, setAllow] = useState('');
  const [deny, setDeny] = useState('');
  const [apiKey, setApiKey] = useState('');
  const [apiSecret, setApiSecret] = useState('');

  const goDry = async () => {
    const ack = await control.setMode('dry_run');
    if (ack.ok) setMode('dry_run');
    setMsg(ack.message);
  };
  const confirmLive = async () => {
    const ack = await control.setMode('live', phrase);
    setMsg(ack.message);
    if (ack.ok) setMode('live');
    setLiveOpen(false);
    setPhrase('');
  };
  const save = () => setMsg('Configuration saved to the session (simulation).');

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
      <Panel title="Trading mode" bodyClassName="p-4 space-y-3">
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted">Current:</span>
          <Badge tone={mode === 'live' ? 'down' : 'neutral'}>{mode === 'live' ? 'LIVE' : 'DRY-RUN'}</Badge>
        </div>
        <div className="flex gap-2">
          <Button variant={mode === 'dry_run' ? 'primary' : 'subtle'} onClick={goDry}>
            Dry-run
          </Button>
          <Button variant={mode === 'live' ? 'danger' : 'subtle'} onClick={() => setLiveOpen(true)}>
            Enable live
          </Button>
        </div>
        <p className="text-2xs text-muted-foreground">
          Live trading requires a signed configuration flag plus this confirmation, and is only
          permitted after a positive-EV 24h backtest.
        </p>
      </Panel>

      <Panel title="Execution" bodyClassName="p-4 grid grid-cols-2 gap-3">
        <Field label="Default execution mode">
          <select value={execMode} onChange={(e) => setExecMode(e.target.value)} className={inputClass}>
            <option value="parallel_ioc">Parallel IOC</option>
            <option value="sequential">Sequential lock-in</option>
          </select>
        </Field>
        <Field label="Max cycle length">
          <select value={cycleLen} onChange={(e) => setCycleLen(Number(e.target.value))} className={inputClass}>
            <option value={3}>3 (triangular)</option>
            <option value={4}>4 (quadrangular)</option>
            <option value={5}>5 (pentagonal)</option>
          </select>
        </Field>
        <Field label="Min edge (bps)">
          <input type="number" value={minEdge} onChange={(e) => setMinEdge(Number(e.target.value))} className={inputClass} />
        </Field>
      </Panel>

      <Panel title="Risk limits" bodyClassName="p-4 grid grid-cols-2 gap-3">
        <Field label="Max notional / trade (USDT)">
          <input type="number" value={maxPerTrade} onChange={(e) => setMaxPerTrade(Number(e.target.value))} className={inputClass} />
        </Field>
        <Field label="Max total open (USDT)">
          <input type="number" value={maxOpen} onChange={(e) => setMaxOpen(Number(e.target.value))} className={inputClass} />
        </Field>
        <Field label="Max daily loss (USDT)">
          <input type="number" value={dailyLoss} onChange={(e) => setDailyLoss(Number(e.target.value))} className={inputClass} />
        </Field>
      </Panel>

      <Panel title="Symbol universe" bodyClassName="p-4 grid grid-cols-2 gap-3">
        <Field label="Allowlist (comma separated)">
          <input value={allow} onChange={(e) => setAllow(e.target.value)} placeholder="BTCUSDT, ETHUSDT" className={inputClass} />
        </Field>
        <Field label="Denylist">
          <input value={deny} onChange={(e) => setDeny(e.target.value)} placeholder="e.g. leveraged tokens" className={inputClass} />
        </Field>
      </Panel>

      <Panel title="API keys (Ed25519)" className="lg:col-span-2" bodyClassName="p-4 space-y-3">
        <div className="grid grid-cols-2 gap-3">
          <Field label="API key">
            <input type="password" value={apiKey} onChange={(e) => setApiKey(e.target.value)} placeholder="••••••••" className={inputClass} />
          </Field>
          <Field label="API secret">
            <input type="password" value={apiSecret} onChange={(e) => setApiSecret(e.target.value)} placeholder="••••••••" className={inputClass} />
          </Field>
        </div>
        <p className="text-2xs text-muted-foreground">
          Keys never leave the backend in production; the frontend proxies all Binance calls. Stored
          encrypted at rest, decrypted only in the execution process.
        </p>
        <div className="flex items-center gap-3">
          <Button variant="primary" onClick={save}>
            Save configuration
          </Button>
          {msg && <span className="text-xs text-muted">{msg}</span>}
        </div>
      </Panel>

      <Modal open={liveOpen} title="Enable live trading" onClose={() => setLiveOpen(false)}>
        <p className="mb-3 text-sm text-muted">
          Type <span className="num font-semibold text-down">{LIVE_PHRASE}</span> to confirm real
          order submission.
        </p>
        <input
          value={phrase}
          onChange={(e) => setPhrase(e.target.value)}
          placeholder={LIVE_PHRASE}
          autoFocus
          className="focus-ring mb-4 w-full rounded-lg border border-border bg-bg px-3 py-2 font-mono text-sm"
        />
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={() => setLiveOpen(false)}>
            Cancel
          </Button>
          <Button variant="danger" disabled={phrase !== LIVE_PHRASE} onClick={confirmLive}>
            Enable live trading
          </Button>
        </div>
      </Modal>
    </div>
  );
}
