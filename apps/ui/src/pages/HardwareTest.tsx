import { useState } from "react";
import { call, saveFile, textFile } from "../api";
import { ErrorBox, SimBanner, Spinner, useAction } from "../components";
import { t } from "../i18n";
import { useApp } from "../state";
import type { LiveSample, Report, Settings } from "../types";
import { protocolName } from "../units";

type Result = "works" | "partial" | "fails" | "not_supported";
type Outcome = Result | "skipped";

interface Row {
  test: string;
  outcome: Outcome;
  notes: string;
  /** Catalog function the result is recorded for (none = summary only). */
  fn?: string;
}

const CONNECT_FN: Record<string, string> = {
  usb_serial: "connect_usb_serial",
  android_usb: "connect_usb_serial",
  bluetooth_classic: "connect_bluetooth_classic",
  wifi: "connect_wifi",
  ethernet: "connect_wifi",
  k_line_cable: "connect_kline_raw",
};

const LABEL: Record<Outcome, string> = {
  works: "Works",
  partial: "Partly",
  fails: "Fails",
  not_supported: "Not supported",
  skipped: "Skipped",
};
const CLS: Record<Outcome, string> = {
  works: "ok",
  partial: "warn",
  fails: "bad",
  not_supported: "",
  skipped: "",
};

function Ask({
  q,
  options,
  onAnswer,
}: {
  q: string;
  options: [string, Outcome][];
  onAnswer: (o: Outcome) => void;
}) {
  return (
    <div style={{ marginTop: 10 }}>
      <p>{q}</p>
      <div className="row">
        {options.map(([label, o]) => (
          <button
            key={label}
            className={`btn small ${o === "works" ? "primary" : ""}`}
            onClick={() => onAnswer(o)}
          >
            {t(label)}
          </button>
        ))}
      </div>
    </div>
  );
}

const YES_NO_SKIP: [string, Outcome][] = [
  ["Yes", "works"],
  ["Partly", "partial"],
  ["No", "fails"],
  ["Did not check", "skipped"],
];

export default function HardwareTest() {
  const { summary, settings, go, setReport, refreshSummary } = useApp();
  const [step, setStep] = useState(0);
  const [rows, setRows] = useState<Row[]>([]);
  const [scan, setScan] = useState<Report | null>(null);
  const [live, setLive] = useState<{
    n: number;
    errors: number;
    rpm: number[];
  } | null>(null);
  const [copied, setCopied] = useState(false);

  const record = async (r: Row) => {
    setRows((x) => [...x.filter((y) => y.test !== r.test), r]);
    if (r.fn && r.outcome !== "skipped") {
      await call("record_test", {
        function: r.fn,
        result: r.outcome,
        notes: `Hardware test: ${r.test}. ${r.notes}`.trim(),
      });
    }
  };
  const next = () => setStep((s) => s + 1);

  const runScan = useAction(async () => {
    const r = await call<Report>("scan");
    setScan(r);
    setReport(r);
    const scanned = r.modules.filter((m) => m.status === "scanned").length;
    await record({
      test: "4a Fault codes read",
      fn: "read_dtcs",
      outcome: scanned > 0 ? "works" : "fails",
      notes: `${scanned} module(s), ${r.dtcs.length} code(s), ${r.communication_errors.length} communication problem(s)`,
    });
    await record({
      test: "4b Readiness",
      fn: "read_readiness",
      outcome: r.readiness.length > 0 ? "works" : "not_supported",
      notes: "",
    });
    if (r.dtcs.length > 0) {
      await record({
        test: "4c Freeze frame",
        fn: "read_freeze_frame",
        outcome: r.freeze_frames.length > 0 ? "works" : "partial",
        notes: r.freeze_frames.length
          ? ""
          : "codes present but no freeze frame",
      });
    }
  });

  const runLive = useAction(async () => {
    let n = 0;
    let errors = 0;
    const rpm: number[] = [];
    const end = Date.now() + 20000;
    while (Date.now() < end) {
      const s = await call<LiveSample[]>("live_read", {
        keys: ["rpm", "coolant_temp", "speed"],
      });
      n += 1;
      errors += s.filter(
        (x) => x.error && !x.error.includes("Not supported"),
      ).length;
      const r = s.find((x) => x.key === "rpm")?.value;
      if (r != null) rpm.push(r);
      setLive({ n, errors, rpm: [...rpm] });
      await new Promise((res) => setTimeout(res, 400));
    }
    return { n, errors, rpm };
  });

  const runReport = useAction(async () => {
    if (!scan) throw new Error(t("Run the scan step first."));
    const id = Number(scan.id);
    const f = await call<{ filename: string; base64: string }>(
      "export_report",
      { id, format: "pdf", include_vin: false },
    );
    await saveFile(f);
    const again = await call<Report>("report", { id });
    const ok = again.dtcs.length === scan.dtcs.length && f.base64.length > 100;
    await record({
      test: "7 Report saved, exported and reopened",
      fn: "reports",
      outcome: ok ? "works" : "fails",
      notes: f.filename,
    });
    next();
  });

  if (!summary) {
    return (
      <>
        <h1>{t("Hardware test")}</h1>
        <p>
          {t(
            "Connect to a real vehicle with a real adapter first. The test then guides you through every Phase 1 function and records the results for your operating system.",
          )}
        </p>
        <button className="btn primary" onClick={() => go("connect")}>
          {t("Connect to Vehicle")}
        </button>
      </>
    );
  }

  const platform = (settings as Settings | null)?.platform ?? "";
  const report = () => {
    const lines = [
      `## OpenDiag hardware test${summary.simulated ? " (SIMULATED practice run)" : ""}`,
      "",
      `- OpenDiag: 0.1.0 · ${platform}`,
      `- Adapter: ${summary.adapter.identity}${summary.adapter.stn_firmware ? ` (${summary.adapter.stn_firmware})` : ""}${summary.adapter.clone_suspected ? " · clone suspected" : ""}`,
      `- Connection: ${summary.link.kind} · ${summary.link.name}`,
      `- Protocol: ${protocolName(summary.protocol)}`,
      `- Vehicle: ${scan?.vehicle.vin?.manufacturer ?? "make unknown"}${scan?.vehicle.calibration_ids.length ? ` · calibration ${scan.vehicle.calibration_ids.map((c) => c[1]).join(", ")}` : ""} (add model and year)`,
      "",
      "| Test | Result | Notes |",
      "| --- | --- | --- |",
      ...rows.map(
        (r) =>
          `| ${r.test} | ${LABEL[r.outcome]} | ${r.notes.replace(/\|/g, "/")} |`,
      ),
    ];
    return lines.join("\n");
  };

  const steps: { title: string; body: JSX.Element }[] = [
    {
      title: "1 · Connection",
      body: (
        <>
          <p>
            {summary.vehicle_connected
              ? t("The vehicle answered.")
              : t(
                  "The adapter is connected but the vehicle did not answer.",
                )}{" "}
            {summary.link.name} · {protocolName(summary.protocol)}
          </p>
          <button
            className="btn primary"
            onClick={async () => {
              await record({
                test: "1 Connect",
                fn: CONNECT_FN[summary.link.kind],
                outcome: summary.vehicle_connected ? "works" : "fails",
                notes: summary.vehicle_error ?? "",
              });
              next();
            }}
          >
            {t("Record and continue")}
          </button>
        </>
      ),
    },
    {
      title: "2 · Adapter capabilities",
      body: (
        <>
          <p>
            <b>{summary.adapter.identity}</b> ·{" "}
            {t("Shows which module answers")}: {summary.adapter.headers} ·{" "}
            {t("Battery voltage")}: {summary.adapter.voltage_reading}
          </p>
          {summary.adapter.notes.map((n) => (
            <p key={n} className="small muted">
              {n}
            </p>
          ))}
          <Ask
            q={t(
              "Does this match the adapter you are using (its name or box)?",
            )}
            options={YES_NO_SKIP}
            onAnswer={async (o) => {
              await record({
                test: "2 Adapter identified",
                fn: "adapter_probe",
                outcome: o,
                notes: summary.adapter.identity,
              });
              next();
            }}
          />
        </>
      ),
    },
    {
      title: "3–4 · Scan",
      body: (
        <>
          <p>
            {t(
              "Ignition on (engine off or idling). The scan reads codes, readiness, freeze frame and measurements.",
            )}
          </p>
          {!scan && (
            <button
              className="btn primary"
              disabled={runScan.busy}
              onClick={() => runScan.run()}
            >
              {runScan.busy ? <Spinner /> : "🩺"} {t("Run the scan")}
            </button>
          )}
          <ErrorBox error={runScan.error} />
          {scan && (
            <>
              <p className="banner info">
                {scan.health.headline}. {t("VIN")}:{" "}
                {scan.vehicle.vin?.vin ?? t("not reported")} · {t("Codes")}:{" "}
                {scan.dtcs.map((d) => d.code).join(", ") || t("none")}
              </p>
              {!rows.some((r) => r.test.startsWith("4d")) ? (
                <Ask
                  q={t(
                    "Does the VIN match the plate on the vehicle (or its papers)?",
                  )}
                  options={YES_NO_SKIP}
                  onAnswer={(o) =>
                    record({
                      test: "4d VIN matches the vehicle",
                      fn: "read_vehicle_info",
                      outcome: o,
                      notes: "",
                    })
                  }
                />
              ) : (
                <Ask
                  q={t(
                    "Do the fault codes match the warning lights, or what another tool shows?",
                  )}
                  options={YES_NO_SKIP}
                  onAnswer={async (o) => {
                    await record({
                      test: "4e Codes match reality",
                      fn: o === "skipped" ? undefined : "read_dtcs",
                      outcome: o,
                      notes: "",
                    });
                    next();
                  }}
                />
              )}
            </>
          )}
        </>
      ),
    },
    {
      title: "5 · Live data",
      body: (
        <>
          <p>
            {t(
              "OpenDiag reads engine speed, coolant temperature and speed for 20 seconds. If it is safe, rev the engine gently once while it runs.",
            )}
          </p>
          {!live || runLive.busy ? (
            <button
              className="btn primary"
              disabled={runLive.busy}
              onClick={() => runLive.run()}
            >
              {runLive.busy ? <Spinner /> : "📈"}{" "}
              {runLive.busy
                ? t("Reading… {n}", { n: live?.n ?? 0 })
                : t("Start the 20-second check")}
            </button>
          ) : (
            <>
              <p>
                {t(
                  "{n} readings, {e} errors, engine speed from {a} to {b} rpm.",
                  {
                    n: live.n,
                    e: live.errors,
                    a: Math.round(
                      Math.min(...(live.rpm.length ? live.rpm : [0])),
                    ),
                    b: Math.round(
                      Math.max(...(live.rpm.length ? live.rpm : [0])),
                    ),
                  },
                )}
              </p>
              <Ask
                q={t(
                  "Did the values look right (and follow the engine when you revved)?",
                )}
                options={YES_NO_SKIP}
                onAnswer={async (o) => {
                  const auto: Outcome =
                    live.errors === 0 && live.n > 10 ? "works" : "partial";
                  await record({
                    test: "5 Live data",
                    fn: "live_data",
                    outcome: o === "skipped" ? auto : o === "works" ? auto : o,
                    notes: `${live.n} readings, ${live.errors} errors`,
                  });
                  next();
                }}
              />
            </>
          )}
          <ErrorBox error={runLive.error} />
        </>
      ),
    },
    {
      title: "6 · Clearing codes (optional)",
      body: (
        <>
          <p>
            {t(
              "Only if a code is present and you accept losing it. Use Fault Codes → Clear codes…, first with the engine running (it must be refused), then with the engine off and the ignition on.",
            )}
          </p>
          <button className="btn small" onClick={() => go("codes")}>
            ⚠ {t("Open Fault Codes")}
          </button>
          <Ask
            q={t("What happened?")}
            options={[
              ["Refused with engine running, cleared with engine off", "works"],
              ["Cleared, but did not test the refusal", "partial"],
              ["Did not clear", "fails"],
              ["Skipped", "skipped"],
            ]}
            onAnswer={async (o) => {
              await record({
                test: "6 Clear codes",
                fn: "clear_emission_dtcs",
                outcome: o,
                notes: "",
              });
              next();
            }}
          />
        </>
      ),
    },
    {
      title: "7 · Report",
      body: (
        <>
          <p>
            {t("Saves the scan as a PDF and reopens it from the database.")}
          </p>
          <button
            className="btn primary"
            disabled={runReport.busy}
            onClick={() => runReport.run()}
          >
            {runReport.busy ? <Spinner /> : "📄"} {t("Export and reopen")}
          </button>
          <ErrorBox error={runReport.error} />
        </>
      ),
    },
    {
      title: "8 · Offline",
      body: (
        <Ask
          q={t(
            "Repeat the scan with flight mode on (Wi-Fi adapters: keep only the adapter's Wi-Fi). Did it work?",
          )}
          options={YES_NO_SKIP}
          onAnswer={async (o) => {
            await record({ test: "8 Works offline", outcome: o, notes: "" });
            next();
          }}
        />
      ),
    },
    {
      title: "9 · Disconnect",
      body: (
        <Ask
          q={t(
            "Start Live Data, unplug the adapter, then plug it back and reconnect. Did the app show an error without freezing, and did reconnecting work?",
          )}
          options={YES_NO_SKIP}
          onAnswer={async (o) => {
            await record({
              test: "9 Disconnect and reconnect",
              outcome: o,
              notes: "",
            });
            await refreshSummary();
            next();
          }}
        />
      ),
    },
  ];

  const done = step >= steps.length;
  return (
    <>
      <h1>{t("Hardware test")}</h1>
      <p className="muted">
        {t(
          "Runs the hardware test protocol with your adapter and vehicle. Each result is saved in the compatibility database for this operating system ({os}).",
          { os: platform },
        )}
      </p>
      <SimBanner show={summary.simulated} />
      {summary.simulated && (
        <p className="small muted">
          {t(
            'Practice run: results are recorded as "simulated" and never count as hardware verification.',
          )}
        </p>
      )}
      <div className="progress" style={{ margin: "12px 0" }}>
        <div
          style={{
            width: `${(Math.min(step, steps.length) / steps.length) * 100}%`,
          }}
        />
      </div>
      {!done && (
        <div className="card">
          <h2 style={{ marginTop: 0 }}>{t(steps[step].title)}</h2>
          {steps[step].body}
          <div className="row" style={{ marginTop: 14 }}>
            <button
              className="btn small ghost"
              disabled={step === 0}
              onClick={() => setStep(step - 1)}
            >
              ← {t("Back")}
            </button>
            <button className="btn small ghost" onClick={next}>
              {t("Skip this step")} →
            </button>
          </div>
        </div>
      )}
      {rows.length > 0 && (
        <div className="card">
          <h2 style={{ marginTop: 0 }}>{t("Results")}</h2>
          <table>
            <tbody>
              {[...rows]
                .sort((a, b) =>
                  a.test.localeCompare(b.test, undefined, { numeric: true }),
                )
                .map((r) => (
                  <tr key={r.test}>
                    <td>{r.test}</td>
                    <td>
                      <span className={`chip ${CLS[r.outcome]}`}>
                        {t(LABEL[r.outcome])}
                      </span>
                    </td>
                    <td className="small muted">{r.notes}</td>
                  </tr>
                ))}
            </tbody>
          </table>
          {done && (
            <>
              <p className="small">
                {t(
                  'Results are saved on this device. To share them: copy the report into a "Compatibility report" issue on GitHub, and attach the snapshot (Compatibility → Offline snapshots → Export).',
                )}
              </p>
              <div className="row">
                <button
                  className="btn primary"
                  onClick={async () => {
                    try {
                      await navigator.clipboard.writeText(report());
                      setCopied(true);
                    } catch {
                      await saveFile(
                        textFile("opendiag-hardware-test.md", report()),
                      );
                    }
                  }}
                >
                  {copied ? t("Copied") : t("Copy report")}
                </button>
                <button
                  className="btn"
                  onClick={() =>
                    saveFile(textFile("opendiag-hardware-test.md", report()))
                  }
                >
                  {t("Save as file")}
                </button>
                <button className="btn" onClick={() => go("compat")}>
                  {t("Compatibility")}
                </button>
              </div>
            </>
          )}
        </div>
      )}
    </>
  );
}
