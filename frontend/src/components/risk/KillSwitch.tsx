import { AlertOctagon } from 'lucide-react';
import { useState } from 'react';
import { Modal } from '@/components/ui/Modal';
import { Button } from '@/components/ui/primitives';
import { useControl } from '@/data/ControlContext';
import { useStore } from '@/store/useStore';

const PHRASE = 'STOP TRADING';

/** The three-channel kill switch (UI channel). Requires a typed confirmation
 *  phrase, then engages the emergency stop and forces dry-run. */
export function KillSwitch({ compact = false }: { compact?: boolean }) {
  const { control } = useControl();
  const emergency = useStore((s) => s.emergencyStopped);
  const setEmergency = useStore((s) => s.setEmergency);
  const setMode = useStore((s) => s.setMode);
  const [open, setOpen] = useState(false);
  const [phrase, setPhrase] = useState('');

  const confirm = async () => {
    if (phrase !== PHRASE) return;
    const ack = await control.emergencyStop('operator kill switch (UI)', true);
    if (ack.ok) {
      setEmergency(true);
      setMode('dry_run');
    }
    setOpen(false);
    setPhrase('');
  };

  return (
    <>
      <Button
        variant="danger"
        size={compact ? 'sm' : 'lg'}
        onClick={() => setOpen(true)}
        className={compact ? '' : 'w-full'}
      >
        <AlertOctagon className="h-4 w-4" />
        {emergency ? 'STOPPED' : 'Kill switch'}
      </Button>

      <Modal open={open} title="Emergency stop" onClose={() => setOpen(false)}>
        <p className="mb-3 text-sm text-muted">
          This halts new order submission immediately and unwinds open positions to USDT. Type{' '}
          <span className="num font-semibold text-down">{PHRASE}</span> to confirm.
        </p>
        <input
          value={phrase}
          onChange={(e) => setPhrase(e.target.value)}
          placeholder={PHRASE}
          autoFocus
          className="focus-ring mb-4 w-full rounded-lg border border-border bg-bg px-3 py-2 font-mono text-sm"
        />
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button variant="danger" disabled={phrase !== PHRASE} onClick={confirm}>
            Engage emergency stop
          </Button>
        </div>
      </Modal>
    </>
  );
}
