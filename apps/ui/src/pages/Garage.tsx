import { useEffect, useState } from 'react';
import { call } from '../api';
import { ErrorBox, useAction } from '../components';
import { t } from '../i18n';
import { useApp } from '../state';
import type { ReportSummary, Vehicle } from '../types';

function Edit({ v, onDone }: { v: Vehicle; onDone: () => void }) {
  const [form, setForm] = useState(v);
  const save = useAction(async () => {
    await call('update_vehicle', { vehicle: form });
    onDone();
  });
  return (
    <div className="stack">
      <div className="grid two">
        <label className="field">
          {t('Name')}
          <input type="text" value={form.label} onChange={(e) => setForm({ ...form, label: e.target.value })} />
        </label>
        <label className="field">
          {t('Make')}
          <input type="text" value={form.make ?? ''} onChange={(e) => setForm({ ...form, make: e.target.value || null })} />
        </label>
        <label className="field">
          {t('Model')}
          <input type="text" value={form.model ?? ''} onChange={(e) => setForm({ ...form, model: e.target.value || null })} />
        </label>
        <label className="field">
          {t('Year')}
          <input type="number" value={form.year ?? ''} onChange={(e) => setForm({ ...form, year: e.target.value ? Number(e.target.value) : null })} />
        </label>
        <label className="field">
          {t('Type')}
          <select value={form.kind ?? ''} onChange={(e) => setForm({ ...form, kind: e.target.value || null })}>
            <option value="">—</option>
            <option value="car">{t('Car')}</option>
            <option value="motorcycle">{t('Motorcycle')}</option>
            <option value="van">{t('Van')}</option>
            <option value="truck">{t('Truck')}</option>
          </select>
        </label>
      </div>
      <label className="field">
        {t('Notes')}
        <textarea value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} />
      </label>
      <div className="row">
        <button className="btn primary" onClick={() => save.run()}>
          {t('Save')}
        </button>
        <button className="btn" onClick={onDone}>
          {t('Cancel')}
        </button>
      </div>
      <ErrorBox error={save.error} />
    </div>
  );
}

export default function Garage() {
  const { go } = useApp();
  const [vehicles, setVehicles] = useState<Vehicle[]>([]);
  const [reports, setReports] = useState<ReportSummary[]>([]);
  const [editing, setEditing] = useState<number | null>(null);
  const load = useAction(async () => {
    setVehicles(await call<Vehicle[]>('vehicles'));
    setReports(await call<ReportSummary[]>('reports', { vehicle_id: null }));
  });
  const del = useAction(async (v: Vehicle) => {
    if (!confirm(t('Delete {name} and all its reports from this device? This cannot be undone.', { name: v.label }))) return;
    await call('delete_vehicle', { id: v.id });
    await load.run();
  });
  useEffect(() => {
    void load.run();
  }, []);
  return (
    <>
      <h1>{t('My Garage')}</h1>
      <p className="muted">{t('Vehicles are added when a scan reads a valid VIN. Everything stays on this device.')}</p>
      <ErrorBox error={load.error ?? del.error} />
      {vehicles.length === 0 && <p className="empty">{t('No vehicles yet. Connect and scan to add one.')}</p>}
      <div className="stack">
        {vehicles.map((v) => {
          const mine = reports.filter((r) => r.vehicle_id === v.id);
          return (
            <div key={v.id} className="card">
              {editing === v.id ? (
                <Edit
                  v={v}
                  onDone={() => {
                    setEditing(null);
                    void load.run();
                  }}
                />
              ) : (
                <>
                  <div className="row between">
                    <h2 style={{ margin: 0 }}>
                      {v.label} {v.simulated && <span className="chip sim">{t('SIMULATED')}</span>}
                    </h2>
                    <div className="row">
                      <button className="btn small" onClick={() => setEditing(v.id)}>
                        {t('Edit')}
                      </button>
                      <button className="btn small" onClick={() => del.run(v)}>
                        🗑
                      </button>
                    </div>
                  </div>
                  <p className="muted small">
                    {[v.make, v.model, v.year, v.kind].filter(Boolean).join(' · ') || t('No details yet')} · VIN <span className="mono">{v.vin ?? '—'}</span>
                  </p>
                  {v.notes && <p>{v.notes}</p>}
                  <h3>{t('History')}</h3>
                  {mine.length === 0 ? (
                    <p className="muted">—</p>
                  ) : (
                    <ul className="tight">
                      {mine.map((r) => (
                        <li key={r.id}>
                          <a href={`#/reports/${r.id}`}>{new Date(r.created_at).toLocaleString()}</a> · {t('{n} code(s)', { n: r.dtc_count })}
                        </li>
                      ))}
                    </ul>
                  )}
                </>
              )}
            </div>
          );
        })}
      </div>
      <button className="btn" style={{ marginTop: 14 }} onClick={() => go('connect')}>
        {t('Connect another vehicle')}
      </button>
    </>
  );
}
